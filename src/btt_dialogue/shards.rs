use std::error::Error;

use ironworks::sestring::SeString;

use super::ast::encode_sestring;
use super::binary::{
    StringPool, align8, checked_u16, checked_u32, checked_u64, pad_to, write_u16, write_u32,
    write_u64,
};
use super::contract::{
    DIALOGUE_MAGIC, DIALOGUE_SCHEMA_VERSION, STRUCTURE_MAGIC, STRUCTURE_SCHEMA_VERSION,
};

pub(super) struct SourceBundleBuilder {
    structure: StructureShardBuilder,
    dialogue: DialogueShardBuilder,
    skipped_empty_keys: usize,
}

impl SourceBundleBuilder {
    pub(super) fn new() -> Self {
        Self {
            structure: StructureShardBuilder::default(),
            dialogue: DialogueShardBuilder::default(),
            skipped_empty_keys: 0,
        }
    }

    pub(super) fn push_structure(
        &mut self,
        record: StructureRecordRef<'_>,
    ) -> Result<(), Box<dyn Error>> {
        self.structure.push(record)
    }

    pub(super) fn push_dialogue(
        &mut self,
        record: DialogueRecordRef<'_>,
    ) -> Result<(), Box<dyn Error>> {
        self.dialogue.push(record)
    }

    pub(super) fn count_skipped_empty_key(&mut self) {
        self.skipped_empty_keys += 1;
    }

    pub(super) fn structure_records(&self) -> usize {
        self.structure.rows.len()
    }

    pub(super) fn dialogue_records(&self) -> usize {
        self.dialogue.rows.len()
    }

    pub(super) fn empty_text_records(&self) -> Result<usize, Box<dyn Error>> {
        self.structure_records()
            .checked_sub(self.dialogue_records())
            .ok_or_else(|| {
                "Dialogue source bundle has more dialogue records than structure records.".into()
            })
    }

    pub(super) fn skipped_empty_keys(&self) -> usize {
        self.skipped_empty_keys
    }

    pub(super) fn structure_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        self.structure.to_bytes()
    }

    pub(super) fn dialogue_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        self.dialogue.to_bytes()
    }
}

#[derive(Clone, Copy)]
pub(super) struct SourceRecordIdentityRef<'a> {
    pub(super) sheet: &'a str,
    pub(super) row: &'a str,
    pub(super) column: &'a str,
    pub(super) key: &'a str,
}

pub(super) struct StructureRecordRef<'a> {
    pub(super) identity: SourceRecordIdentityRef<'a>,
    pub(super) has_text: bool,
}

pub(super) struct DialogueRecordRef<'a> {
    pub(super) identity: SourceRecordIdentityRef<'a>,
    pub(super) value: SeString<'a>,
}

#[derive(Default)]
struct StructureShardBuilder {
    pool: StringPool,
    rows: Vec<StructureRow>,
}

impl StructureShardBuilder {
    fn push(&mut self, record: StructureRecordRef<'_>) -> Result<(), Box<dyn Error>> {
        self.rows.push(StructureRow {
            key: self.pool.add(record.identity.key)?,
            sheet: self.pool.add(record.identity.sheet)?,
            row: self.pool.add(record.identity.row)?,
            column: self.pool.add(record.identity.column)?,
            flags: u32::from(record.has_text),
        });
        Ok(())
    }

    fn to_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let row_size = 20usize;
        let header_size = 72usize;
        let rows_offset = header_size;
        let row_bytes_len = self.rows.len() * row_size;
        let (string_offsets, string_bytes) = self.pool.to_buffers();
        let string_offsets_offset = align8(rows_offset + row_bytes_len);
        let string_bytes_offset = string_offsets_offset + string_offsets.len();

        let mut output = Vec::with_capacity(string_bytes_offset + string_bytes.len());
        output.extend_from_slice(STRUCTURE_MAGIC);
        write_u16(&mut output, STRUCTURE_SCHEMA_VERSION);
        write_u16(
            &mut output,
            checked_u16(header_size, "structure header size")?,
        );
        write_u32(&mut output, 0);
        write_u32(
            &mut output,
            checked_u32(self.rows.len(), "structure row count")?,
        );
        write_u32(
            &mut output,
            checked_u32(self.pool.len(), "structure string count")?,
        );
        write_u32(&mut output, checked_u32(row_size, "structure row size")?);
        write_u32(&mut output, 0);
        write_u64(
            &mut output,
            checked_u64(rows_offset, "structure rows offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(string_offsets_offset, "structure string offsets offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(string_bytes_offset, "structure string bytes offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(string_bytes.len(), "structure string bytes length")?,
        );

        for row in &self.rows {
            write_u32(&mut output, row.key);
            write_u32(&mut output, row.sheet);
            write_u32(&mut output, row.row);
            write_u32(&mut output, row.column);
            write_u32(&mut output, row.flags);
        }

        pad_to(&mut output, string_offsets_offset);
        output.extend_from_slice(&string_offsets);
        output.extend_from_slice(&string_bytes);
        Ok(output)
    }
}

struct StructureRow {
    key: u32,
    sheet: u32,
    row: u32,
    column: u32,
    flags: u32,
}

#[derive(Default)]
struct DialogueShardBuilder {
    pool: StringPool,
    rows: Vec<DialogueRow>,
}

impl DialogueShardBuilder {
    fn push(&mut self, record: DialogueRecordRef<'_>) -> Result<(), Box<dyn Error>> {
        let mut ast = Vec::new();
        encode_sestring(record.value, &mut ast, &mut self.pool)?;
        self.rows.push(DialogueRow {
            key: self.pool.add(record.identity.key)?,
            sheet: self.pool.add(record.identity.sheet)?,
            row: self.pool.add(record.identity.row)?,
            column: self.pool.add(record.identity.column)?,
            ast,
        });
        Ok(())
    }

    fn to_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let row_size = 16usize;
        let header_size = 88usize;
        let rows_offset = header_size;
        let ast_offsets_offset = rows_offset + self.rows.len() * row_size;
        let ast_bytes_offset = ast_offsets_offset + (self.rows.len() + 1) * 8;

        let mut ast_offsets = Vec::with_capacity((self.rows.len() + 1) * 8);
        let mut ast_bytes = Vec::new();
        for row in &self.rows {
            write_u64(
                &mut ast_offsets,
                checked_u64(ast_bytes.len(), "dialogue AST byte offset")?,
            );
            ast_bytes.extend_from_slice(&row.ast);
        }
        write_u64(
            &mut ast_offsets,
            checked_u64(ast_bytes.len(), "dialogue AST byte length")?,
        );

        let (string_offsets, string_bytes) = self.pool.to_buffers();
        let string_offsets_offset = align8(ast_bytes_offset + ast_bytes.len());
        let string_bytes_offset = string_offsets_offset + string_offsets.len();

        let mut output = Vec::with_capacity(string_bytes_offset + string_bytes.len());
        output.extend_from_slice(DIALOGUE_MAGIC);
        write_u16(&mut output, DIALOGUE_SCHEMA_VERSION);
        write_u16(
            &mut output,
            checked_u16(header_size, "dialogue header size")?,
        );
        write_u32(&mut output, 0);
        write_u32(
            &mut output,
            checked_u32(self.rows.len(), "dialogue row count")?,
        );
        write_u32(
            &mut output,
            checked_u32(self.pool.len(), "dialogue string count")?,
        );
        write_u32(&mut output, checked_u32(row_size, "dialogue row size")?);
        write_u32(&mut output, 0);
        write_u64(
            &mut output,
            checked_u64(rows_offset, "dialogue rows offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(ast_offsets_offset, "dialogue AST offsets offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(ast_bytes_offset, "dialogue AST bytes offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(string_offsets_offset, "dialogue string offsets offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(string_bytes_offset, "dialogue string bytes offset")?,
        );
        write_u64(
            &mut output,
            checked_u64(string_bytes.len(), "dialogue string bytes length")?,
        );

        for row in &self.rows {
            write_u32(&mut output, row.key);
            write_u32(&mut output, row.sheet);
            write_u32(&mut output, row.row);
            write_u32(&mut output, row.column);
        }

        output.extend_from_slice(&ast_offsets);
        output.extend_from_slice(&ast_bytes);
        pad_to(&mut output, string_offsets_offset);
        output.extend_from_slice(&string_offsets);
        output.extend_from_slice(&string_bytes);
        Ok(output)
    }
}

struct DialogueRow {
    key: u32,
    sheet: u32,
    row: u32,
    column: u32,
    ast: Vec<u8>,
}
