use std::{
    fs,
    io::Result,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap_num::maybe_hex;

/// File content for extractions
pub struct FileContent(Vec<u8>);

impl FileContent {
    pub fn try_new(path: PathBuf) -> Result<Self> {
        Ok(Self(fs::read(path)?))
    }

    /// Get reference to the file content
    pub fn content(&self) -> &[u8] {
        &self.0
    }

    /// Get mutable reference to the file content
    pub fn content_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }

    /// Get reference to the vec
    pub fn as_vec(&self) -> &Vec<u8> {
        &self.0
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for FileContent {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl TryFrom<FileContentSpec> for FileContent {
    type Error = std::io::Error;

    fn try_from(value: FileContentSpec) -> std::result::Result<Self, Self::Error> {
        Self::try_new(value.0)
    }
}

impl Deref for FileContent {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.content()
    }
}

/// Helper for the clap
#[derive(Clone)]
pub struct FileContentSpec(PathBuf);

impl FromStr for FileContentSpec {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let path = PathBuf::from(s);

        Ok(Self(path))
    }
}

impl Deref for FileContentSpec {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// File for uploading to the device
pub struct UploadFile {
    content: FileContent,
    addr: u32,
}

impl UploadFile {
    pub fn try_new(path: PathBuf, addr: u32) -> Result<Self> {
        Ok(Self {
            content: FileContent::try_new(path)?,
            addr,
        })
    }

    pub fn from_content(content: FileContent, addr: u32) -> Self {
        Self { content, addr }
    }

    /// Get reference to the file content
    pub fn content(&self) -> &[u8] {
        self.content.content()
    }

    /// Get mutable reference to the file content
    pub fn content_mut(&mut self) -> &mut [u8] {
        self.content.content_mut()
    }

    /// Get reference to the vec
    pub fn as_vec(&self) -> &Vec<u8> {
        self.content.as_vec()
    }

    /// Get file upload address
    pub fn upload_address(&self) -> u32 {
        self.addr
    }
}

impl TryFrom<UploadFileSpec> for UploadFile {
    type Error = std::io::Error;

    fn try_from(value: UploadFileSpec) -> std::result::Result<Self, Self::Error> {
        Self::try_new(value.file.0, value.addr)
    }
}

impl Deref for UploadFile {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.content()
    }
}

/// Helper for the clap
#[derive(Clone)]
pub struct UploadFileSpec {
    file: FileContentSpec,
    addr: u32,
}

impl FromStr for UploadFileSpec {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let (path, addr) = s.rsplit_once('@').ok_or("expected PATH@ADDRESS")?;

        Ok(Self {
            file: FileContentSpec::from_str(path)?,
            addr: maybe_hex(addr)?,
        })
    }
}

impl Deref for UploadFileSpec {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}
