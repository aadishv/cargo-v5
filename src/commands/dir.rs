use chrono::{TimeZone, Utc};
use std::io::{self, Write};
use std::time::Duration;
use vex_v5_serial::packets::factory::{
    FactoryEnablePacket, FactoryEnablePayload, FactoryEnableReplyPacket,
};

use vex_v5_serial::packets::file::ExtensionType;
use vex_v5_serial::timestamp::J2000_EPOCH;
use vex_v5_serial::{
    connection::{serial::SerialConnection, Connection},
    packets::file::{
        FileVendor, GetDirectoryEntryPacket, GetDirectoryEntryPayload,
        GetDirectoryEntryReplyPacket, GetDirectoryFileCountPacket, GetDirectoryFileCountPayload,
        GetDirectoryFileCountReplyPacket,
    },
};

use humansize::{format_size, BINARY};
use tabwriter::TabWriter;

use crate::errors::CliError;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: u32,
    pub load_address: u32,
    pub vendor: FileVendor,
    pub extension_type: Option<String>,
    pub timestamp: Option<String>,
    pub version: Option<String>,
    pub crc: u32,
}

fn vendor_prefix(vid: FileVendor) -> &'static str {
    match vid {
        FileVendor::User => "user/",
        FileVendor::Sys => "sys_/",
        FileVendor::Dev1 => "rmsh/",
        FileVendor::Dev2 => "pros/",
        FileVendor::Dev3 => "mwrk/",
        FileVendor::Dev4 => "deva/",
        FileVendor::Dev5 => "devb/",
        FileVendor::Dev6 => "devc/",
        FileVendor::VexVm => "vxvm/",
        FileVendor::Vex => "vex_/",
        FileVendor::Undefined => "test/",
    }
}

pub async fn get_file_entries(connection: &mut SerialConnection) -> Result<Vec<FileInfo>, CliError> {
    const USEFUL_VIDS: [FileVendor; 11] = [
        FileVendor::User,
        FileVendor::Sys,
        FileVendor::Dev1,
        FileVendor::Dev2,
        FileVendor::Dev3,
        FileVendor::Dev4,
        FileVendor::Dev5,
        FileVendor::Dev6,
        FileVendor::VexVm,
        FileVendor::Vex,
        FileVendor::Undefined,
    ];

    connection
        .packet_handshake::<FactoryEnableReplyPacket>(
            Duration::from_millis(500),
            1,
            FactoryEnablePacket::new(FactoryEnablePayload::new()),
        )
        .await?;

    let mut entries = Vec::new();

    for vid in USEFUL_VIDS {
        let file_count = connection
            .packet_handshake::<GetDirectoryFileCountReplyPacket>(
                Duration::from_millis(500),
                1,
                GetDirectoryFileCountPacket::new(GetDirectoryFileCountPayload {
                    vendor: vid,
                    option: 0,
                }),
            )
            .await?;

        for n in 0..file_count.payload {
            if let Some(entry) = connection
                .packet_handshake::<GetDirectoryEntryReplyPacket>(
                    Duration::from_millis(500),
                    1,
                    GetDirectoryEntryPacket::new(GetDirectoryEntryPayload {
                        file_index: n as u8,
                        unknown: 0,
                    }),
                )
                .await?
                .payload
            {
                let file_info = FileInfo {
                    path: format!("{}{}", vendor_prefix(vid), entry.file_name),
                    size: entry.size,
                    load_address: entry.load_address,
                    vendor: vid,
                    extension_type: entry
                        .metadata
                        .as_ref()
                        .map(|m| match m.extension_type {
                            ExtensionType::Binary => "binary".to_string(),
                            ExtensionType::EncryptedBinary => "encrypted".to_string(),
                            ExtensionType::Vm => "vm".to_string(),
                        }),
                    timestamp: entry
                        .metadata
                        .as_ref()
                        .map(|m| Utc
                            .timestamp_millis_opt((J2000_EPOCH as i64 + m.timestamp as i64) * 1000)
                            .unwrap()
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()),
                    version: entry
                        .metadata
                        .as_ref()
                        .map(|m| format!(
                            "{}.{}.{}.b{}",
                            m.version.major, m.version.minor, m.version.build, m.version.beta
                        )),
                    crc: entry.crc,
                };
                entries.push(file_info);
            }
        }
    }

    Ok(entries)
}

pub async fn dir(connection: &mut SerialConnection) -> Result<(), CliError> {
    let mut tw = TabWriter::new(io::stdout());
    let entries = get_file_entries(connection).await?;

    // Warm the completions cache
    let file_paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    super::completions::write_cache(&file_paths);

    write!(
        &mut tw,
        "\x1B[1mName\tSize\tLoad Address\tVendor\tType\tTimestamp\tVersion\tCRC32\n\x1B[0m"
    )?;

    for entry in entries {
        writeln!(
            &mut tw,
            "{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}",
            entry.path,
            format_size(entry.size, BINARY),
            if entry.load_address == u32::MAX {
                "-".to_string()
            } else {
                format!("{:#x}", entry.load_address)
            },
            entry.vendor,
            entry.extension_type.unwrap_or_else(|| "system".to_string()),
            entry.timestamp.unwrap_or_else(|| "-".to_string()),
            entry.version.unwrap_or_else(|| "-".to_string()),
            if entry.crc == u32::MAX {
                "-".to_string()
            } else {
                format!("{:#x}", entry.crc)
            },
        )?;
    }

    tw.flush()?;
    Ok(())
}
