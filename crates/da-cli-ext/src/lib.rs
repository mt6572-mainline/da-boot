use hacc::Image;

/// parse first image partition
///
/// returns either Some(name, parsed image) or None
pub fn maybe_image(data: &[u8]) -> Option<(&str, &[u8])> {
    Image::partitions_from_slice(data)
        .next()
        .map(|part| (part.header.name(), part.content))
}
