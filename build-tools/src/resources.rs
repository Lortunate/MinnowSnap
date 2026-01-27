use anyhow::Result;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::{fs, io::Cursor, path::Path};
use walkdir::WalkDir;

struct ResourceFile {
    alias: Option<String>,
    path: String,
}

struct ResourceBlock {
    prefix: String,
    files: Vec<ResourceFile>,
}

fn write_files<W: std::io::Write>(writer: &mut Writer<W>, files: &[ResourceFile]) -> Result<()> {
    for file in files {
        let mut elem = BytesStart::new("file");
        if let Some(alias) = &file.alias {
            elem.push_attribute(("alias", alias.as_str()));
        }
        writer.write_event(Event::Start(elem.borrow()))?;
        writer.write_event(Event::Text(BytesText::new(&file.path)))?;
        writer.write_event(Event::End(BytesEnd::new("file")))?;
    }
    Ok(())
}

fn write_block<W: std::io::Write>(writer: &mut Writer<W>, block: &ResourceBlock) -> Result<()> {
    let mut elem = BytesStart::new("qresource");
    elem.push_attribute(("prefix", block.prefix.as_str()));
    writer.write_event(Event::Start(elem.borrow()))?;
    write_files(writer, &block.files)?;
    writer.write_event(Event::End(BytesEnd::new("qresource")))?;
    Ok(())
}

pub fn update_resources(qrc_path: &Path, resource_dir: &Path) -> Result<()> {
    let mut root_files = Vec::new();
    if resource_dir.exists() {
        for entry in WalkDir::new(resource_dir).sort_by_file_name() {
            let entry = entry?;
            if entry.file_type().is_file() {
                let name = entry.file_name().to_string_lossy();
                if name.starts_with('.') || name.ends_with(".qrc") || name.ends_with(".ts") || name == ".DS_Store" {
                    continue;
                }
                let path = entry.path();
                let rel_path = path.to_string_lossy().replace("\\", "/");
                let rel_path = rel_path.trim_start_matches("./").to_string();
                root_files.push(rel_path);
            }
        }
    }

    let blocks = [
        ResourceBlock {
            prefix: "/".to_string(),
            files: root_files.into_iter().map(|p| ResourceFile { alias: None, path: p }).collect(),
        },
        ResourceBlock {
            prefix: "/qt/qml/com/lortunate/minnow".to_string(),
            files: vec![
                ResourceFile {
                    alias: Some("qmldir".to_string()),
                    path: "qml/qmldir".to_string(),
                },
                ResourceFile {
                    alias: Some("AppTheme.qml".to_string()),
                    path: "qml/AppTheme.qml".to_string(),
                },
            ],
        },
    ];

    let content = if qrc_path.exists() {
        fs::read_to_string(qrc_path)?
    } else {
        "<RCC>\n</RCC>".to_string()
    };

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 4);
    let mut buf = Vec::new();

    let mut written_blocks = vec![false; blocks.len()];
    let mut inside_managed_resource = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"qresource" => {
                let mut current_block_idx = None;
                for attr in e.attributes() {
                    let attr = attr?;
                    if attr.key.as_ref() == b"prefix" {
                        let prefix = String::from_utf8_lossy(&attr.value).into_owned();
                        if let Some(idx) = blocks.iter().position(|b| b.prefix == prefix) {
                            current_block_idx = Some(idx);
                        }
                    }
                }

                if let Some(idx) = current_block_idx {
                    inside_managed_resource = true;
                    written_blocks[idx] = true;

                    let mut elem = BytesStart::new("qresource");
                    elem.push_attribute(("prefix", blocks[idx].prefix.as_str()));
                    writer.write_event(Event::Start(elem.borrow()))?;
                    write_files(&mut writer, &blocks[idx].files)?;
                } else {
                    writer.write_event(Event::Start(e.clone()))?;
                }
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"qresource" => {
                if inside_managed_resource {
                    inside_managed_resource = false;
                }
                writer.write_event(Event::End(e.clone()))?;
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"RCC" => {
                for (idx, &written) in written_blocks.iter().enumerate() {
                    if !written {
                        write_block(&mut writer, &blocks[idx])?;
                    }
                }
                writer.write_event(Event::End(e.clone()))?;
            }
            Ok(Event::Eof) => break,
            Ok(e) => {
                if !inside_managed_resource {
                    writer.write_event(e)?;
                }
            }
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
        }
        buf.clear();
    }

    let result = writer.into_inner().into_inner();
    let result_str = String::from_utf8(result)?;

    if qrc_path.exists() {
        let current_content = fs::read_to_string(qrc_path)?;
        if current_content == result_str {
            return Ok(());
        }
    }

    fs::write(qrc_path, result_str)?;
    println!("Updated {}", qrc_path.display());
    Ok(())
}
