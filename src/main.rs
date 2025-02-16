use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::exit;

#[repr(C)]
struct Header {
    symbol_offset: u32,
    symbol_length: u32,
    relocation_offset: u32,
    relocation_length: u32,
    code_offset: u32,
}

impl Header {
    fn from_slice(slice: &[u8]) -> Self {
        let mut iter = slice.iter();
        Header {
            symbol_offset: read_u32(&mut iter),
            symbol_length: read_u32(&mut iter),
            relocation_offset: read_u32(&mut iter),
            relocation_length: read_u32(&mut iter),
            code_offset: read_u32(&mut iter),
        }
    }
}

fn read_u32(iter: &mut std::slice::Iter<u8>) -> u32 {
    let bytes: Vec<u8> = iter.take(4).cloned().collect();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

struct ObjectFile {
    symbols: HashMap<String, u32>,
    relocations: HashMap<String, Vec<u32>>,
    code: Vec<u8>,
}

fn read_object_file(path: &str) -> io::Result<ObjectFile> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let header = Header::from_slice(&buffer[0..std::mem::size_of::<Header>()]);
    let symbols = read_symbols(&buffer, header.symbol_offset, header.symbol_length);
    let relocations = read_relocations(&buffer, header.relocation_offset, header.relocation_length);
    let code = buffer
        [header.code_offset as usize..]
        .to_vec();

    Ok(ObjectFile {
        symbols,
        relocations,
        code,
    })
}

fn read_symbols(buffer: &[u8], offset: u32, length: u32) -> HashMap<String, u32> {
    let mut symbols = HashMap::new();
    let mut pos = offset as usize;

    for _ in 0..length {
        let name_len = buffer[pos] as usize;
        pos += 1;
        let name = String::from_utf8(buffer[pos..pos + name_len].to_vec()).unwrap();
        pos += name_len;

        let address = u32::from_le_bytes([
            buffer[pos],
            buffer[pos + 1],
            buffer[pos + 2],
            buffer[pos + 3],
        ]);
        pos += 4;

        symbols.insert(name, address);
    }

    symbols
}

fn read_relocations(buffer: &[u8], offset: u32, length: u32) -> HashMap<String, Vec<u32>> {
    let mut relocations: HashMap<String, Vec<u32>> = HashMap::new();
    let mut pos = offset as usize;

    for _ in 0..length {
        let symbol_name_length = buffer[pos] as usize;
        pos += 1;

        let symbol_name =
            String::from_utf8(buffer[pos..pos + symbol_name_length].to_vec()).unwrap();
        pos += symbol_name_length;

        let locations_length = u32::from_le_bytes([
            buffer[pos],
            buffer[pos + 1],
            buffer[pos + 2],
            buffer[pos + 3],
        ]);
        pos += 4;

        for _ in 0..locations_length {
            let relocation = u32::from_le_bytes([
                buffer[pos],
                buffer[pos + 1],
                buffer[pos + 2],
                buffer[pos + 3],
            ]);
            pos += 4;

            if let Some(vec) = relocations.get_mut(&symbol_name) {
                vec.push(relocation);
            } else {
                relocations.insert(symbol_name.clone(), vec![relocation]);
            }
        }
    }
    relocations
}

fn link_object_files(paths: &[String], output_path: &str) -> io::Result<()> {
    let mut combined_symbols = HashMap::new();
    let mut combined_relocations = HashMap::<String, Vec<u32>>::new();
    let mut combined_code = Vec::new();
    let mut base_address = 0;

    for path in paths {
        let obj_file = read_object_file(path)?;

        for (name, address) in obj_file.symbols {
            combined_symbols.insert(name, address + base_address);
        }

        for (symbol, mut addresses) in obj_file.relocations {
            for address in addresses.iter_mut() {
                *address += base_address;
            }
            if let Some(existing_addresses) = combined_relocations.get_mut(&symbol) {
                existing_addresses.extend(addresses);
            } else {
                combined_relocations.insert(symbol, addresses);
            }
        }

        combined_code.extend(obj_file.code);
        base_address = combined_code.len() as u32;
    }

    if let Err(e) = apply_relocations(&mut combined_code, &combined_symbols, &combined_relocations)
    {
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
    relocations: &HashMap<String, Vec<u32>>,
) -> Result<(), String> {
    let mut unresolved_symbols = Vec::new();

    for (symbol, addresses) in relocations {
        if let Some(&symbol_address) = symbols.get(symbol) {
            let bytes = symbol_address.to_le_bytes();
            for &address in addresses {
                code[address as usize..address as usize + 4].copy_from_slice(&bytes);
            }
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
