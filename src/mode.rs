/// How much of the repository the packer is allowed to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackMode {
    /// Budget solver with L0–L4 decisions.
    Auto,
    /// Every symbol at L0. This is the full-dump baseline.
    Full,
    /// Every symbol at L3. This is the name-map baseline.
    Names,
}
