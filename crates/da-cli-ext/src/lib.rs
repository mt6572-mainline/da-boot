use hacc::{Image, Preloader, TryRead};

/// parse first image partition
///
/// returns either Some(name, parsed image) or None
pub fn maybe_image(data: &[u8]) -> Option<(&str, &[u8])> {
    Image::partitions_from_slice(data)
        .next()
        .map(|part| (part.header.name(), part.content))
}

/// parse preloader
///
/// returns either Some(base addr, parsed image) or None
pub fn maybe_preloader(data: &[u8]) -> Option<(u32, &[u8])> {
    Preloader::try_read(&data).ok().map(|pl| {
        let pl_jump = pl.gfh().file_info().load_addr() + pl.gfh().file_info().jump_offset();

        // wow. mtk bullshit is everywhere.
        let sliced = if data.starts_with(b"EMMC_BOOT") {
            &data[0xb00..]
        } else if data.starts_with(b"MMM") {
            &data[0x300..]
        } else {
            unreachable!("Junk preloader");
        };

        (pl_jump, sliced)
    })
}
