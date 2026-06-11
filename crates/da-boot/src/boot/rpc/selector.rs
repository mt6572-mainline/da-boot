use std::{borrow::Cow, fs, io::Write, process::Command};

use anyhow::{Context, Result};
use da_params::{MAGIC, PayloadParams};
use da_patcher::{
    Extract,
    lk::{mt_part_generic_read::MtPartGenericRead, mt_part_get_partition::MtPartGetPartition},
    preloader::bldr_jump::BldrJump,
};
use da_protocol::{HookId, LKRunnerParams, Message, PreloaderRunnerParams, Protocol, Response};
use hacc::Image;
use memchr::memmem;
use tempfile::NamedTempFile;

use crate::{
    BootArgument, BootMode, Port, State,
    boot::{give_me_bytes_please, rpc::ext::HostExtensions},
    repl::run_repl,
    run_payload,
};

pub fn run_rpc_preloader(state: &mut State, mut port: Port) -> Result<()> {
    let da_addr = state.soc.da_dram_addr();
    let mut payload = pl_payload()?;

    for image in &state.upload {
        let addr = image.upload_address();
        if state
            .params
            .blacklist_reloc(addr..addr + image.len() as u32 + 1)
            .is_err()
        {
            anyhow::bail!("Failed blacklisting range: {addr:#x}");
        }

        println!("Reserved memory: {addr:#x}");
    }

    if let Some(ref image) = state.lk {
        let addr = image.file.upload_address();
        if state
            .params
            .blacklist_reloc(addr..addr + image.file.len() as u32 + 1)
            .is_err()
        {
            anyhow::bail!("Failed blacklisting range: {addr:#x}");
        }

        println!("Reserved memory: {addr:#x}");
    }

    let mut payload = payload.to_mut();
    inject_params(&state, &mut payload)?;
    run_payload(da_addr, &payload, &mut port)?;

    let mut protocol = start_rpc(port)?;
    println!("Got loader sync !");

    let bldr_jump = BldrJump::new(&state.preloader.analyzer)
        .extract()
        .context("Failed to get bldr_jump fn ptr")?;
    let pl_params = PreloaderRunnerParams::new(bldr_jump);

    protocol.send_message(Message::SetParams(da_protocol::ParamsType::Preloader(
        pl_params,
    )))?;
    if !protocol.read_response().is_ok_and(|r| r.is_ack()) {
        anyhow::bail!("Error on setting preloader params");
    }

    let mut jump = state.jump_addr;

    for image in &state.upload {
        let addr = image.upload_address();
        println!("Uploading image to {addr:#x}");
        protocol
            .upload(addr, &image)
            .context("Failed uploading image")?;
        protocol.send_message(Message::BlacklistRange(addr..addr + image.len() as u32 + 1))?;
        if !protocol.read_response().is_ok_and(|r| r.is_ack()) {
            anyhow::bail!("Failed blacklisting {addr:#x}");
        }
    }

    if let Some(ref lk) = state.lk {
        let addr = lk.file.upload_address();
        println!("Uploading LK to {addr:#x}");
        protocol
            .upload(addr, &lk.file)
            .context("Failed uploading lk")?;
        protocol.send_message(Message::BlacklistRange(
            addr..addr + lk.file.len() as u32 + 1,
        ))?;
        if !protocol.read_response().is_ok_and(|r| r.is_ack()) {
            anyhow::bail!("Failed blacklisting {addr:#x}");
        }

        println!("Preparing boot argument for LK");
        protocol.upload(
            lk.boot_argument_addr,
            give_me_bytes_please(&BootArgument::lk(state.lk_mode)),
        )?;
    }

    match state.mode {
        BootMode::BootROM => (),
        BootMode::Preloader => {
            if let Some(ref lk) = state.lk {
                jump = lk.file.upload_address();
                println!("Jump address set to LK entry ({jump:#x})");
            }
        }
        BootMode::LK { .. } => {
            let image = state.lk.as_ref().unwrap();

            let start = {
                if let Some(ref kernel) = state.kernel {
                    let mut kernel_image = Image::default();
                    kernel_image
                        .add_partition(
                            "KERNEL",
                            kernel,
                            hacc::ImageKind::Ap(hacc::ImageAPKind::APBin),
                        )
                        .context("Failed to add KERNEL partition")?;

                    let ramdisk_data = if let Some(ref ramdisk) = state.ramdisk {
                        ramdisk.content()
                    } else {
                        &[0; 1024]
                    };

                    let mut ramdisk_image = Image::default();
                    ramdisk_image
                        .add_partition(
                            "ROOTFS",
                            ramdisk_data,
                            hacc::ImageKind::Ap(hacc::ImageAPKind::APBin),
                        )
                        .context("Failed to add ROOTFS partition")?;

                    let mut kernel =
                        NamedTempFile::new().context("Failed to create kernel temp file")?;
                    kernel
                        .write_all(&kernel_image.data)
                        .context("Failed to write kernel to the file")?;

                    let mut ramdisk =
                        NamedTempFile::new().context("Failed to create ramdisk temp file")?;
                    ramdisk
                        .write_all(&ramdisk_image.data)
                        .context("Failed to write ramdisk to the file")?;

                    let out = NamedTempFile::new().context("Failed to create output file")?;

                    let cmd = Command::new("mkbootimg")
                        .arg("--kernel")
                        .arg(kernel.path())
                        .arg("--ramdisk")
                        .arg(ramdisk.path())
                        .arg("-o")
                        .arg(out.path())
                        .output()
                        .context("Failed to run mkbootimg")?;

                    if !cmd.status.success() {
                        anyhow::bail!("Failed to create boot image");
                    }

                    let boot_img = fs::read(out.path()).context("Failed to read boot.img")?;
                    let size = boot_img.len() as u32;
                    protocol.send_message(Message::GetFreeRange { size })?;
                    let Response::Range(Some(start)) = protocol.read_response()? else {
                        anyhow::bail!("Failed to request free range for {size} bytes");
                    };

                    println!("Allocated {size} bytes at {start:#x}, sending boot.img");
                    protocol
                        .upload(start, &boot_img)
                        .context("Failed to send boot.img")?;

                    protocol.send_message(Message::BlacklistRange(start..start + size + 1))?;
                    if !protocol.read_response().is_ok_and(|r| r.is_ack()) {
                        anyhow::bail!("Failed to blacklist boot.img range");
                    }
                    println!("Reserved memory: {start:#x} (boot.img)");

                    start
                } else {
                    let start = state.upload.get(0).context("No input")?.upload_address();
                    println!("Using first uploaded image as boot.img (at {start:#x})");
                    start
                }
            };

            let mt_part_generic_read = MtPartGenericRead::new(&image.analyzer)
                .extract()
                .context("Failed to extract mt_part_generic_read")?;
            let mt_part_get_partition = MtPartGetPartition::new(&image.analyzer)
                .extract()
                .context("Failed to extract mt_part_get_partition")?;
            let lk_params =
                LKRunnerParams::new(mt_part_generic_read | 1, mt_part_get_partition | 1, start);
            protocol.send_message(Message::SetParams(da_protocol::ParamsType::LK(lk_params)))?;
            if !protocol.read_response().is_ok_and(|r| r.is_ack()) {
                anyhow::bail!("Failed to set LK params");
            }

            println!("Setting up LK hooks");
            protocol.send_message(Message::hook(HookId::MtPartGenericRead))?;
            if !protocol.read_response()?.is_ack() {
                anyhow::bail!("Error on replacing mt_part_generic_read");
            }

            println!(
                "Replaced mt_part_generic_read ({mt_part_generic_read:#x}), helper: mt_part_get_partition ({mt_part_get_partition:#x})"
            );

            jump = image.file.upload_address();
            println!("Jump address set to LK entry ({jump:#x})");
        }
        BootMode::REPL => return run_repl(protocol),
    }

    println!("Jump to {jump:#x}");
    protocol.send_message(Message::jump(
        jump,
        state.lk.as_ref().map(|lk| lk.boot_argument_addr),
        Some(250),
    ))?;
    if protocol.read_response().is_ok_and(|r| r.is_nack()) {
        anyhow::bail!("Error on jump");
    } else {
        Ok(())
    }
}

pub fn start_rpc(port: Port) -> Result<Protocol<Port>> {
    let mut protocol = Protocol::new(port);
    protocol.start()?;
    Ok(protocol)
}

pub fn inject_params(context: &State, payload: &mut [u8]) -> Result<()> {
    let start = memmem::find(payload, &MAGIC.to_le_bytes()).context("Failed to get magic")?;
    let new = give_me_bytes_please(&context.params);
    println!("Write params ({:#x} bytes) at {start:#x} offset", new.len());
    payload[start..start + size_of::<PayloadParams>()].clone_from_slice(new);
    Ok(())
}

pub fn brom_payload() -> Result<Cow<'static, [u8]>> {
    #[cfg(not(feature = "static"))]
    {
        Ok(Cow::Owned(std::fs::read(
            "target/armv7a-none-eabi/nostd/brom",
        )?))
    }
    #[cfg(feature = "static")]
    {
        Ok(Cow::Borrowed(include_bytes!(
            "../../../../../target/armv7a-none-eabi/nostd/pl"
        )))
    }
}

pub fn pl_payload() -> Result<Cow<'static, [u8]>> {
    #[cfg(not(feature = "static"))]
    {
        Ok(Cow::Owned(std::fs::read(
            "target/armv7a-none-eabi/nostd/pl",
        )?))
    }
    #[cfg(feature = "static")]
    {
        Ok(Cow::Borrowed(include_bytes!(
            "../../../../../target/armv7a-none-eabi/nostd/pl"
        )))
    }
}
