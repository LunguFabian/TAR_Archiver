use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::fs;
use std::fs::symlink_metadata;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;
#[derive(Debug)]
pub struct UStarHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    modification_time: [u8; 12],
    checksum: [u8; 8],
    type_flag: [u8; 1],
    link_name: [u8; 100],
    ustar: [u8; 6],
    version: [u8; 2],
    user_name: [u8; 32],
    group_name: [u8; 32],
    device_major: [u8; 8],
    device_minor: [u8; 8],
    prefix: [u8; 155],
    padding: [u8; 12],
}
impl UStarHeader {
    fn new() -> Self {
        UStarHeader {
            name: [0; 100],
            mode: [0; 8],
            uid: [0; 8],
            gid: [0; 8],
            size: [0; 12],
            modification_time: [0; 12],
            checksum: [0; 8],
            type_flag: [0; 1],
            link_name: [0; 100],
            ustar: [0; 6],
            version: [0; 2],
            user_name: [0; 32],
            group_name: [0; 32],
            device_major: [0; 8],
            device_minor: [0; 8],
            prefix: [0; 155],
            padding: [0; 12],
        }
    }
    fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(512);

        bytes.extend_from_slice(&self.name);
        bytes.extend_from_slice(&self.mode);
        bytes.extend_from_slice(&self.uid);
        bytes.extend_from_slice(&self.gid);
        bytes.extend_from_slice(&self.size);
        bytes.extend_from_slice(&self.modification_time);
        bytes.extend_from_slice(&self.checksum);
        bytes.push(self.type_flag[0]);
        bytes.extend_from_slice(&self.link_name);
        bytes.extend_from_slice(&self.ustar);
        bytes.extend_from_slice(&self.version);
        bytes.extend_from_slice(&self.user_name);
        bytes.extend_from_slice(&self.group_name);
        bytes.extend_from_slice(&self.device_major);
        bytes.extend_from_slice(&self.device_minor);
        bytes.extend_from_slice(&self.prefix);
        bytes.extend_from_slice(&self.padding);

        bytes
    }
}
fn calculate_checksum(header: &UStarHeader) -> u32 {
    let mut checksum: u32 = 0;
    for byte in header.as_bytes() {
        checksum += byte as u32;
    }
    checksum
}
fn create_header(
    path: &Path,
    parent_path: &Path,
    inode_map: &mut HashMap<u64, String>,
) -> Result<UStarHeader, std::io::Error> {
    let mut header = UStarHeader::new();
    let metadata = symlink_metadata(path)?;
    let path_name = path
        .strip_prefix(parent_path.to_str().unwrap())
        .unwrap()
        .to_str()
        .unwrap();

    let path_bytes = path_name.as_bytes();
    if metadata.is_dir() {
        let mut dir_bytes = path_bytes.to_vec();
        dir_bytes.push(b'/');
        if dir_bytes.len() > 100 {
            let (prefix, name) = dir_bytes.split_at(dir_bytes.len() - 100);
            header.name[..name.len()].copy_from_slice(name);
            header.prefix[..prefix.len()].copy_from_slice(prefix);
        } else {
            header.name[..dir_bytes.len()].copy_from_slice(&dir_bytes);
        }
    } else if path_bytes.len() > 100 {
        let (prefix, name) = path_bytes.split_at(path_bytes.len() - 100);
        header.name[..name.len()].copy_from_slice(name);
        header.prefix[..prefix.len()].copy_from_slice(prefix);
    } else {
        header.name[..path_bytes.len()].copy_from_slice(path_bytes);
    }

    if let Some(original_path) = inode_map.get(&metadata.ino()) {
        header.type_flag[0] = b'1';
        let link_bytes = original_path.as_bytes();
        header.link_name[..link_bytes.len()].copy_from_slice(link_bytes);
    } else if metadata.is_file() {
        header.type_flag[0] = b'0';
    } else if metadata.is_symlink() {
        header.type_flag[0] = b'2';
        let link_target = fs::read_link(path)?;
        let link_target_str = link_target.to_str().unwrap();
        let link_bytes = link_target_str.as_bytes();
        header.link_name[..link_bytes.len()].copy_from_slice(link_bytes);
    } else if metadata.file_type().is_block_device() || metadata.file_type().is_char_device() {
        let device_major = format!("{:0>7o}\0", metadata.rdev() >> 8);
        let device_minor = format!("{:0>7o}\0", metadata.rdev());
        header.device_major[..device_major.len()].copy_from_slice(device_major.as_bytes());
        header.device_minor[..device_minor.len()].copy_from_slice(device_minor.as_bytes());
        header.type_flag[0] = if metadata.file_type().is_block_device() {
            b'4'
        } else {
            b'3'
        };
    } else if metadata.is_dir() {
        header.type_flag[0] = b'5';
    } else if metadata.file_type().is_fifo() {
        header.type_flag[0] = b'6';
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Unsupported file type",
        ));
    }

    let mode = format!("{:0>7o}\0", metadata.mode() & 0o777);
    header.mode[..mode.len()].copy_from_slice(mode.as_bytes());

    let uid = format!("{:0>7o}\0", metadata.uid());
    let gid = format!("{:0>7o}\0", metadata.gid());
    header.uid[..uid.len()].copy_from_slice(uid.as_bytes());
    header.gid[..gid.len()].copy_from_slice(gid.as_bytes());

    let mut uname: String = "".to_string();
    let mut content = fs::read_to_string("/etc/passwd")?;
    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 && metadata.uid() == parts[2].parse::<u32>().unwrap() {
            uname = parts[0].to_string();
        }
    }
    let mut gname: String = "".to_string();
    content = fs::read_to_string("/etc/group")?;
    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 && metadata.gid() == parts[2].parse::<u32>().unwrap() {
            gname = parts[0].to_string();
        }
    }
    header.user_name[..uname.len()].copy_from_slice(uname.as_bytes());
    header.group_name[..gname.len()].copy_from_slice(gname.as_bytes());

    let size = if metadata.is_dir() || metadata.is_file() {
        metadata.len()
    } else {
        0
    };
    let file_size = format!("{:0>11o}\0", size);
    header.size[..file_size.len()].copy_from_slice(file_size.as_bytes());

    let mtime = metadata.mtime();
    let mtime_str = format!("{:o}\0", mtime);
    header.modification_time[..mtime_str.len()].copy_from_slice(mtime_str.as_bytes());

    header.ustar.copy_from_slice(b"ustar\0");
    header.version.copy_from_slice(b"00");

    header.checksum.fill(b' ');
    let checksum_str = format!("{:06o}\0", calculate_checksum(&header));
    header.checksum[..checksum_str.len()].copy_from_slice(checksum_str.as_bytes());

    Ok(header)
}
fn add_to_archive(
    file_path: &Path,
    parent_path: &Path,
    tar_buffer: &mut Vec<u8>,
    inode_map: &mut HashMap<u64, String>,
) -> Result<(), std::io::Error> {
    if symlink_metadata(file_path)?.is_file() {
        let metadata = symlink_metadata(file_path)?;
        let inode = metadata.ino();

        if inode_map.contains_key(&inode) {
            let header = create_header(file_path, parent_path, inode_map)?;
            tar_buffer.write_all(&header.as_bytes())?;
        } else {
            let header = create_header(file_path, parent_path, inode_map)?;
            tar_buffer.write_all(&header.as_bytes())?;

            inode_map.insert(
                inode,
                file_path
                    .strip_prefix(parent_path.to_str().unwrap())
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            );
            let mut file = fs::File::open(file_path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            tar_buffer.write_all(&buffer)?;

            let padding = (512 - (metadata.len() % 512)) % 512;
            tar_buffer.write_all(&vec![0; padding as usize])?;
        }
    } else if symlink_metadata(file_path)?.is_dir() {
        let header = create_header(file_path, parent_path, inode_map)?;
        tar_buffer.write_all(&header.as_bytes())?;

        for entry in fs::read_dir(file_path)? {
            let entry = entry?;
            let path = entry.path();
            add_to_archive(&path, parent_path, tar_buffer, inode_map)?;
        }
    } else {
        let header = create_header(file_path, parent_path, inode_map)?;
        tar_buffer.write_all(&header.as_bytes())?;
    }
    Ok(())
}
pub fn create_archive(
    base_path_name: &str,
    archive_name: &str,
    compress: bool,
) -> Result<(), std::io::Error> {
    let archive_file_name = if compress {
        format!("{}.tar.gz", archive_name)
    } else {
        format!("{}.tar", archive_name)
    };

    let mut tar_buffer = Vec::new();
    let mut inode_map: HashMap<u64, String> = HashMap::new();

    let path = Path::new(base_path_name);
    let parent = path.parent().unwrap();
    match add_to_archive(path, parent, &mut tar_buffer, &mut inode_map) {
        Ok(()) => {
            tar_buffer.extend_from_slice(&[0; 512]);
            tar_buffer.extend_from_slice(&[0; 512]);

            if compress {
                let archive_file = fs::File::create(archive_file_name)?;
                let mut encoder = GzEncoder::new(archive_file, Compression::default());
                encoder.write_all(&tar_buffer)?;
                encoder.finish()?;
            } else {
                let mut archive_file = fs::File::create(archive_file_name)?;
                archive_file.write_all(&tar_buffer)?;
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}
