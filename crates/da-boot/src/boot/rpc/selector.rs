use std::{borrow::Cow, fs, io::Write, process::Command};

use acon::SoC;
use anyhow::{Context, Result};
use da_params::{MAGIC, PayloadParams};
use da_patcher::{
    Extract,
    lk::{
        get_part::GetPart, mt_part_generic_read::MtPartGenericRead,
        mt_part_get_partition::MtPartGetPartition,
    },
    preloader::bldr_jump::BldrJump,
};
use da_protocol::{HookId, LKRunnerParams, Message, PreloaderRunnerParams, Protocol, Response};
use hacc::Image;
use memchr::memmem;
use tempfile::NamedTempFile;

use crate::{
    BootMode, Port, State,
    boot::{give_me_bytes_please, lk_arg::get_for_soc, rpc::ext::HostExtensions},
    repl::run_repl,
    run_payload,
};

pub fn run_rpc_preloader(state: &mut State, mut port: Port) -> Result<()> {
    let (bldr_jump, da_addr) = BldrJump::new(&state.preloader.analyzer)
        .extract()
        .context("Failed to get bldr_jump fn ptr")?;
    let pl_params = PreloaderRunnerParams::new(bldr_jump);

    let mut payload = pl_payload()?;

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

    let (mut bootarg_base, mut bootarg_size) = (0, 0);
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
        let bootarg = get_for_soc(
            state.soc,
            state.lk_mode,
            state.dram_size_per_rank,
            state.dram_ranks,
        );
        let bytes = bootarg.as_bytes();
        bootarg_size = bytes.len() as u32;

        // dynamically passed in R4
        protocol.send_message(Message::GetFreeRange { size: bootarg_size })?;
        let Response::Range(Some(start)) = protocol.read_response()? else {
            anyhow::bail!("Failed to request free range for {bootarg_size} bytes");
        };
        println!("Boot argument will be set to {start:#x}");

        bootarg_base = start;
        protocol.send_message(Message::BlacklistRange(start..start + bootarg_size + 1))?;
        if !protocol.read_response().is_ok_and(|r| r.is_ack()) {
            anyhow::bail!("Failed blacklisting {addr:#x}");
        }

        println!("Reserved memory: {start:#x} (LK boot argument)");

        protocol.upload(start, bytes)?;
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
                        .arg("--base")
                        .arg("0x40000000")
                        .arg("--kernel_offset")
                        .arg("0x8000")
                        .arg("--ramdisk_offset")
                        .arg("0x4000000")
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
            let mt_part_get_partition = match &state.soc {
                SoC::MT6572 => MtPartGetPartition::new(&image.analyzer)
                    .extract()
                    .context("Failed to extract mt_part_get_partition")?,
                SoC::MT6595 => GetPart::new(&image.analyzer)
                    .extract()
                    .context("Failed to extract get_part")?,
                _ => unreachable!(),
            };
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
        Some(bootarg_base as u32),
        Some(bootarg_size as u32),
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
