//! ELF32 objects built byte by byte, as tests/target2_abs32_test.py built them.

pub const SHT_PROGBITS: u32 = 1;
const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_REL: u32 = 9;
pub const R_ARM_ABS32: u32 = 2;
pub const R_ARM_THM_CALL: u32 = 10;
pub const R_ARM_TARGET2: u32 = 41;

/// The ELF header fields a refusal test changes; the default is what GCCE writes.
pub struct Ident {
    pub class: u8,
    pub data: u8,
    pub e_type: u16,
    pub machine: u16,
}

pub const GCCE: Ident = Ident {
    class: 1,
    data: 1,
    e_type: 1,
    machine: 40,
};

/// An ELF32 relocatable object: a null section, then `sections`, each (name, type,
/// payload, entsize), then .shstrtab. Payloads follow the 52-byte header, the section
/// header table comes last.
pub fn elf(sections: &[(&str, u32, Vec<u8>, u32)], ident: &Ident) -> Vec<u8> {
    let mut names = vec![0u8];
    for (name, ..) in sections {
        names.extend_from_slice(name.as_bytes());
        names.push(0);
    }
    names.extend_from_slice(b".shstrtab\0");
    let mut all: Vec<(&str, u32, Vec<u8>, u32)> = sections.to_vec();
    all.push((".shstrtab", SHT_STRTAB, names, 0));
    let mut body = Vec::new();
    let mut headers = vec![0u8; 40];
    let mut name_off = 1u32;
    for (name, kind, payload, entsize) in &all {
        let offset = 52 + body.len() as u32;
        body.extend_from_slice(payload);
        for word in [
            name_off,
            *kind,
            0,
            0,
            offset,
            payload.len() as u32,
            0,
            0,
            4,
            *entsize,
        ] {
            headers.extend_from_slice(&word.to_le_bytes());
        }
        name_off += name.len() as u32 + 1;
    }
    let shoff = 52 + body.len() as u32;
    let count = (headers.len() / 40) as u16;
    let mut out = b"\x7fELF".to_vec();
    out.extend_from_slice(&[ident.class, ident.data, 1]);
    out.extend_from_slice(&[0; 9]);
    let big = ident.data != 1;
    let h = |v: u16| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let w = |v: u32| {
        if big {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    out.extend(h(ident.e_type).into_iter().chain(h(ident.machine)));
    for word in [1, 0, 0, shoff, 0x0500_0000] {
        out.extend(w(word));
    }
    for half in [52, 0, 0, 40, count, count - 1] {
        out.extend(h(half));
    }
    out.extend(body);
    out.extend(headers);
    out
}

/// SHT_REL entries (offset, symbol, type).
pub fn rel(entries: &[(u32, u32, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (offset, sym, kind) in entries {
        out.extend(
            offset
                .to_le_bytes()
                .into_iter()
                .chain(((sym << 8) | kind).to_le_bytes()),
        );
    }
    out
}

/// SHT_RELA entries (offset, symbol, type, addend).
pub fn rela(entries: &[(u32, u32, u32, i32)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (offset, sym, kind, addend) in entries {
        out.extend(
            offset
                .to_le_bytes()
                .into_iter()
                .chain(((sym << 8) | kind).to_le_bytes()),
        );
        out.extend(addend.to_le_bytes());
    }
    out
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
}

/// (offset, symbol, type) of every entry of the section at `index`.
pub fn relocations(data: &[u8], index: usize, step: usize) -> Vec<(u32, u32, u32)> {
    let header = u32_at(data, 0x20) as usize + index * 40;
    let (off, size) = (
        u32_at(data, header + 16) as usize,
        u32_at(data, header + 20) as usize,
    );
    (off..off + size)
        .step_by(step)
        .map(|p| {
            (
                u32_at(data, p),
                u32_at(data, p + 4) >> 8,
                u32_at(data, p + 4) & 0xff,
            )
        })
        .collect()
}
