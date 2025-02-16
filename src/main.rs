use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::exit;

#[repr(C)]
struct Header {
    symbol_offset: u32,
    relocation_offset: u32,
    code_offset: u32,
}

impl Header {
    fn from_slice(slice: &[u8]) -> Self {
        let mut header = Header {
            symbol_offset: 0,
            relocation_offset: 0,
            code_offset: 0,
        };
        header.symbol_offset = u32::from_le_bytes([slice[5], slice[6], slice[7], slice[8]]);
        header.relocation_offset = u32::from_le_bytes([slice[9], slice[10], slice[11], slice[12]]);
        header.code_offset = u32::from_le_bytes([slice[13], slice[14], slice[15], slice[16]]);
        header
    }
}

struct ObjectFile {
    symbols: HashMap<String, u32>,
    relocations: Vec<(String, u32)>,
    code: Vec<u8>,
}

fn read_object_file(path: &str) -> io::Result<ObjectFile> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let header = Header::from_slice(&buffer[0..17]);
    let symbols = read_symbols(&buffer, header.symbol_offset);
    let relocations = read_relocations(&buffer, header.relocation_offset);
    let code = buffer[header.code_offset as usize..].to_vec();

    Ok(ObjectFile {
        symbols,
        relocations,
        code,
    })
}

fn read_symbols(buffer: &[u8], offset: u32) -> HashMap<String, u32> {
    let mut symbols = HashMap::new();
    let buffer_len = buffer.len();
    if (offset as usize) + 4 > buffer_len {
        panic!("Invalid symbol table offset, {offset}, buffer length {buffer_len}");
    }
    let count = u32::from_le_bytes([
        buffer[offset as usize],
        buffer[offset as usize + 1],
        buffer[offset as usize + 2],
        buffer[offset as usize + 3],
    ]);
    let mut pos = offset as usize + 4;
    for i in 0..count {
        if pos >= buffer_len {
            panic!("Unexpected end of buffer while reading symbol name length at symbol {}", i);
        }
        let name_len = buffer[pos] as usize;
        pos += 1;
        if pos + name_len > buffer_len {
            panic!("Unexpected end of buffer while reading symbol name at symbol {}", i);
        }
        let name = String::from_utf8(buffer[pos..pos + name_len].to_vec()).unwrap();
        pos += name_len;
        if pos + 4 > buffer_len {
            panic!("Unexpected end of buffer while reading symbol address at symbol {}", i);
        }
        let address = u32::from_le_bytes([
            buffer[pos],
            buffer[pos + 1],
            buffer[pos + 2],
            buffer[pos + 3],
        ]);
        pos += 4;
        println!("Read symbol: {} at address: {}", name, address); // Debug print
        symbols.insert(name, address);
    }
    symbols
}
fn read_relocations(buffer: &[u8], offset: u32) -> Vec<(String, u32)> {
    let mut relocations = Vec::new();
    let buffer_len = buffer.len();
    if (offset as usize) + 4 > buffer_len {
        panic!("Invalid relocation table offset");
    }
    let count = u32::from_le_bytes([
        buffer[offset as usize],
        buffer[offset as usize + 1],
        buffer[offset as usize + 2],
        buffer[offset as usize + 3],
    ]);
    let mut pos = offset as usize + 4;
    for _ in 0..count {
        if pos + 4 > buffer_len {
            panic!("Unexpected end of buffer while reading symbol index");
        }
        let symbol_index = u32::from_le_bytes([
            buffer[pos],
            buffer[pos + 1],
            buffer[pos + 2],
            buffer[pos + 3],
        ]);
        pos += 4;
        if pos + 4 > buffer_len {
            panic!("Unexpected end of buffer while reading relocation address");
        }
        let address = u32::from_le_bytes([
            buffer[pos],
            buffer[pos + 1],
            buffer[pos + 2],
            buffer[pos + 3],
        ]);
        pos += 4;
        relocations.push((symbol_index.to_string(), address));
    }
    relocations
}

fn link_object_files(paths: &[String], output_path: &str) -> io::Result<()> {
    let mut combined_symbols = HashMap::new();
    let mut combined_relocations = Vec::new();
    let mut combined_code = Vec::new();
    let mut base_address = 0;

    for path in paths {
        let obj_file = read_object_file(path)?;

        for (name, address) in obj_file.symbols {
            combined_symbols.insert(name, address + base_address);
        }

        for (symbol, address) in obj_file.relocations {
            combined_relocations.push((symbol, address + base_address));
        }

        combined_code.extend(obj_file.code);
        base_address = combined_code.len() as u32;
    }

    if let Err(e) = apply_relocations(&mut combined_code, &combined_symbols, &combined_relocations) {
        eprintln!("Linking failed: {}", e);
        exit(1);
    }

    let mut output = File::create(output_path)?;
    output.write_all(&combined_code)?;

    Ok(())
}

fn apply_relocations(
    code: &mut Vec<u8>,
    symbols: &HashMap<String, u32>,
    relocations: &[(String, u32)],
) -> Result<(), String> {
    let mut unresolved_symbols = Vec::new();

    for (symbol, address) in relocations {
        if let Some(&symbol_address) = symbols.get(symbol) {
            let bytes = symbol_address.to_le_bytes();
            code[*address as usize..*address as usize + 4].copy_from_slice(&bytes);
        } else {
            unresolved_symbols.push(symbol.clone());
        }
    }

    if !unresolved_symbols.is_empty() {
        return Err(format!(
            "Unresolved symbols: {}",
            unresolved_symbols.join(", ")
        ));
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        eprintln!("Usage: <program> <object_files> -o <output_name>");
        exit(1);
    }

    let mut output_name = String::new();
    let mut object_files = Vec::new();

    let mut i = 1;
    while i < args.len() {
        if args[i] == "-o" {
            if i + 1 >= args.len() {
                eprintln!("Error: No output name specified after '-o'.");
                exit(1);
            }
            if !output_name.is_empty() {
                eprintln!("Error: Multiple output names specified.");
                exit(1);
            }
            output_name = args[i + 1].clone();
            i += 2;
        } else {
            object_files.push(args[i].clone());
            i += 1;
        }
    }

    if output_name.is_empty() {
        eprintln!("Error: No output name specified. Use '-o <output_name>'.");
        exit(1);
    }

    if object_files.is_empty() {
        eprintln!("Error: No input object files specified.");
        exit(1);
    }

    // Call the linker function with the collected object files and output name
    if let Err(e) = link_object_files(&object_files, &output_name) {
        eprintln!("Linking failed: {}", e);
        exit(1);
    }
}