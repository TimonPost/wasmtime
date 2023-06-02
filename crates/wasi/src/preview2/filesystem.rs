use crate::preview2::{HostInputStream, HostOutputStream, Table, TableError};
use std::sync::Arc;

bitflags::bitflags! {
    pub struct FilePerms: usize {
        const READ = 0b1;
        const WRITE = 0b10;
    }
}

pub(crate) struct File {
    pub file: Arc<cap_std::fs::File>,
    pub perms: FilePerms,
}

impl File {
    pub fn new(file: cap_std::fs::File, perms: FilePerms) -> Self {
        Self {
            file: Arc::new(file),
            perms,
        }
    }
}
pub(crate) trait TableFsExt {
    fn push_file(&mut self, file: File) -> Result<u32, TableError>;
    fn delete_file(&mut self, fd: u32) -> Result<File, TableError>;
    fn is_file(&self, fd: u32) -> bool;
    fn get_file(&self, fd: u32) -> Result<&File, TableError>;

    fn push_dir(&mut self, dir: Dir) -> Result<u32, TableError>;
    fn delete_dir(&mut self, fd: u32) -> Result<Dir, TableError>;
    fn is_dir(&self, fd: u32) -> bool;
    fn get_dir(&self, fd: u32) -> Result<&Dir, TableError>;
}

impl TableFsExt for Table {
    fn push_file(&mut self, file: File) -> Result<u32, TableError> {
        self.push(Box::new(file))
    }
    fn delete_file(&mut self, fd: u32) -> Result<File, TableError> {
        self.delete(fd)
    }
    fn is_file(&self, fd: u32) -> bool {
        self.is::<File>(fd)
    }
    fn get_file(&self, fd: u32) -> Result<&File, TableError> {
        self.get(fd)
    }

    fn push_dir(&mut self, dir: Dir) -> Result<u32, TableError> {
        self.push(Box::new(dir))
    }
    fn delete_dir(&mut self, fd: u32) -> Result<Dir, TableError> {
        self.delete(fd)
    }
    fn is_dir(&self, fd: u32) -> bool {
        self.is::<Dir>(fd)
    }
    fn get_dir(&self, fd: u32) -> Result<&Dir, TableError> {
        self.get(fd)
    }
}

bitflags::bitflags! {
    pub struct DirPerms: usize {
        const READ = 0b1;
        const MUTATE = 0b10;
    }
}

pub(crate) struct Dir {
    pub dir: cap_std::fs::Dir,
    pub perms: DirPerms,
    pub file_perms: FilePerms,
}

impl Dir {
    pub fn new(dir: cap_std::fs::Dir, perms: DirPerms, file_perms: FilePerms) -> Self {
        Dir {
            dir,
            perms,
            file_perms,
        }
    }
}

pub(crate) struct FileInputStream {
    file: Arc<cap_std::fs::File>,
    position: u64,
}
impl FileInputStream {
    pub fn new(file: Arc<cap_std::fs::File>, position: u64) -> Self {
        Self { file, position }
    }
}

#[async_trait::async_trait]
impl HostInputStream for FileInputStream {
    async fn read(&mut self, buf: &mut [u8]) -> anyhow::Result<(u64, bool)> {
        use system_interface::fs::FileIoExt;
        let (n, end) = read_result(self.file.read_at(buf, self.position))?;
        self.position = self.position.wrapping_add(n);
        Ok((n, end))
    }
    async fn read_vectored<'a>(
        &mut self,
        bufs: &mut [std::io::IoSliceMut<'a>],
    ) -> anyhow::Result<(u64, bool)> {
        use system_interface::fs::FileIoExt;
        let (n, end) = read_result(self.file.read_vectored_at(bufs, self.position))?;
        self.position = self.position.wrapping_add(n);
        Ok((n, end))
    }
    fn is_read_vectored(&self) -> bool {
        use system_interface::fs::FileIoExt;
        self.file.is_read_vectored_at()
    }
    async fn readable(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

pub(crate) fn read_result(r: Result<usize, std::io::Error>) -> Result<(u64, bool), std::io::Error> {
    match r {
        Ok(0) => Ok((0, true)),
        Ok(n) => Ok((n as u64, false)),
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Ok((0, false)),
        Err(e) => Err(e),
    }
}

pub(crate) struct FileOutputStream {
    file: Arc<cap_std::fs::File>,
    position: u64,
}
impl FileOutputStream {
    pub fn new(file: Arc<cap_std::fs::File>, position: u64) -> Self {
        Self { file, position }
    }
}

#[async_trait::async_trait]
impl HostOutputStream for FileOutputStream {
    /// Write bytes. On success, returns the number of bytes written.
    async fn write(&mut self, buf: &[u8]) -> anyhow::Result<u64> {
        use system_interface::fs::FileIoExt;
        let n = self.file.write_at(buf, self.position)? as i64 as u64;
        self.position = self.position.wrapping_add(n);
        Ok(n)
    }

    /// Vectored-I/O form of `write`.
    async fn write_vectored<'a>(&mut self, bufs: &[std::io::IoSlice<'a>]) -> anyhow::Result<u64> {
        use system_interface::fs::FileIoExt;
        let n = self.file.write_vectored_at(bufs, self.position)? as i64 as u64;
        self.position = self.position.wrapping_add(n);
        Ok(n)
    }

    /// Test whether vectored I/O writes are known to be optimized in the
    /// underlying implementation.
    fn is_write_vectored(&self) -> bool {
        use system_interface::fs::FileIoExt;
        self.file.is_write_vectored_at()
    }

    /// Test whether this stream is writable.
    async fn writable(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

pub(crate) struct FileAppendStream {
    file: Arc<cap_std::fs::File>,
}
impl FileAppendStream {
    pub fn new(file: Arc<cap_std::fs::File>) -> Self {
        Self { file }
    }
}

#[async_trait::async_trait]
impl HostOutputStream for FileAppendStream {
    /// Write bytes. On success, returns the number of bytes written.
    async fn write(&mut self, buf: &[u8]) -> anyhow::Result<u64> {
        use system_interface::fs::FileIoExt;
        Ok(self.file.append(buf)? as i64 as u64)
    }

    /// Vectored-I/O form of `write`.
    async fn write_vectored<'a>(&mut self, bufs: &[std::io::IoSlice<'a>]) -> anyhow::Result<u64> {
        use system_interface::fs::FileIoExt;
        let n = self.file.append_vectored(bufs)? as i64 as u64;
        Ok(n)
    }

    /// Test whether vectored I/O writes are known to be optimized in the
    /// underlying implementation.
    fn is_write_vectored(&self) -> bool {
        use system_interface::fs::FileIoExt;
        self.file.is_write_vectored_at()
    }

    /// Test whether this stream is writable.
    async fn writable(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
