use std::borrow::Cow;
use std::error::Error;

use ironworks::sestring::{Expression, MacroKind, MacroPayload, Payload, SeString};

use super::binary::{StringPool, checked_u32, write_u8, write_u32};
use super::contract::{
    AST_BINARY, AST_MACRO, AST_SESTRING, AST_STACK_COLOR, AST_TEXT, AST_TIME_PART, AST_U32,
    AST_UNARY, AST_UNHANDLED_EXPRESSION, AST_UNKNOWN_EXPRESSION,
};

pub(super) fn encode_sestring(
    value: SeString,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    let payloads = value.payloads().collect::<Result<Vec<_>, _>>()?;
    write_u8(output, AST_SESTRING);
    write_u32(
        output,
        checked_u32(payloads.len(), "SeString payload count")?,
    );
    for payload in payloads {
        encode_payload(payload, output, pool)?;
    }
    Ok(())
}

fn encode_payload(
    payload: Payload,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    match payload {
        Payload::Text(text) => {
            write_u8(output, AST_TEXT);
            write_u32(output, pool.add(text.as_utf8()?)?);
        }
        Payload::Macro(payload) => encode_macro(payload, output, pool)?,
    }
    Ok(())
}

fn encode_macro(
    payload: MacroPayload,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    let expressions = payload.expressions().collect::<Result<Vec<_>, _>>()?;
    write_u8(output, AST_MACRO);
    write_u32(output, pool.add(macro_kind_name(payload.kind()).as_ref())?);
    write_u32(
        output,
        checked_u32(expressions.len(), "macro expression count")?,
    );
    for expression in expressions {
        encode_expression(expression, output, pool)?;
    }
    Ok(())
}

fn encode_expression(
    expression: Expression,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    match expression {
        Expression::U32(value) => {
            write_u8(output, AST_U32);
            write_u32(output, value);
        }
        Expression::SeString(value) => encode_sestring(value, output, pool)?,
        Expression::Millisecond => encode_time_part("Millisecond", output, pool)?,
        Expression::Second => encode_time_part("Second", output, pool)?,
        Expression::Minute => encode_time_part("Minute", output, pool)?,
        Expression::Hour => encode_time_part("Hour", output, pool)?,
        Expression::Day => encode_time_part("Day", output, pool)?,
        Expression::Weekday => encode_time_part("Weekday", output, pool)?,
        Expression::Month => encode_time_part("Month", output, pool)?,
        Expression::Year => encode_time_part("Year", output, pool)?,
        Expression::StackColor => write_u8(output, AST_STACK_COLOR),
        Expression::LocalNumber(value) => encode_unary("LocalNumber", *value, output, pool)?,
        Expression::GlobalNumber(value) => encode_unary("GlobalNumber", *value, output, pool)?,
        Expression::LocalString(value) => encode_unary("LocalString", *value, output, pool)?,
        Expression::GlobalString(value) => encode_unary("GlobalString", *value, output, pool)?,
        Expression::Ge(left, right) => encode_binary("Ge", *left, *right, output, pool)?,
        Expression::Gt(left, right) => encode_binary("Gt", *left, *right, output, pool)?,
        Expression::Le(left, right) => encode_binary("Le", *left, *right, output, pool)?,
        Expression::Lt(left, right) => encode_binary("Lt", *left, *right, output, pool)?,
        Expression::Eq(left, right) => encode_binary("Eq", *left, *right, output, pool)?,
        Expression::Ne(left, right) => encode_binary("Ne", *left, *right, output, pool)?,
        Expression::Unknown(value) => {
            write_u8(output, AST_UNKNOWN_EXPRESSION);
            write_u32(output, u32::from(value));
        }
        other => {
            write_u8(output, AST_UNHANDLED_EXPRESSION);
            write_u32(output, pool.add(&format!("{other:?}"))?);
        }
    }
    Ok(())
}

fn encode_time_part(
    name: &str,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    write_u8(output, AST_TIME_PART);
    write_u32(output, pool.add(name)?);
    Ok(())
}

fn encode_unary(
    name: &str,
    value: Expression,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    write_u8(output, AST_UNARY);
    write_u32(output, pool.add(name)?);
    encode_expression(value, output, pool)
}

fn encode_binary(
    name: &str,
    left: Expression,
    right: Expression,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    write_u8(output, AST_BINARY);
    write_u32(output, pool.add(name)?);
    encode_expression(left, output, pool)?;
    encode_expression(right, output, pool)
}

fn macro_kind_name(kind: MacroKind) -> Cow<'static, str> {
    let name = match kind {
        MacroKind::SetResetTime => "SetResetTime",
        MacroKind::SetTime => "SetTime",
        MacroKind::If => "If",
        MacroKind::Switch => "Switch",
        MacroKind::PcName => "PcName",
        MacroKind::IfPcGender => "IfPcGender",
        MacroKind::IfPcName => "IfPcName",
        MacroKind::Josa => "Josa",
        MacroKind::Josaro => "Josaro",
        MacroKind::IfSelf => "IfSelf",
        MacroKind::NewLine => "NewLine",
        MacroKind::Wait => "Wait",
        MacroKind::Icon => "Icon",
        MacroKind::Color => "Color",
        MacroKind::EdgeColor => "EdgeColor",
        MacroKind::ShadowColor => "ShadowColor",
        MacroKind::SoftHyphen => "SoftHyphen",
        MacroKind::Key => "Key",
        MacroKind::Scale => "Scale",
        MacroKind::Bold => "Bold",
        MacroKind::Italic => "Italic",
        MacroKind::Edge => "Edge",
        MacroKind::Shadow => "Shadow",
        MacroKind::NonBreakingSpace => "NonBreakingSpace",
        MacroKind::Icon2 => "Icon2",
        MacroKind::Hyphen => "Hyphen",
        MacroKind::Num => "Num",
        MacroKind::Hex => "Hex",
        MacroKind::Kilo => "Kilo",
        MacroKind::Byte => "Byte",
        MacroKind::Sec => "Sec",
        MacroKind::Time => "Time",
        MacroKind::Float => "Float",
        MacroKind::Link => "Link",
        MacroKind::Sheet => "Sheet",
        MacroKind::String => "String",
        MacroKind::Caps => "Caps",
        MacroKind::Head => "Head",
        MacroKind::Split => "Split",
        MacroKind::HeadAll => "HeadAll",
        MacroKind::Fixed => "Fixed",
        MacroKind::Lower => "Lower",
        MacroKind::JaNoun => "JaNoun",
        MacroKind::EnNoun => "EnNoun",
        MacroKind::DeNoun => "DeNoun",
        MacroKind::FrNoun => "FrNoun",
        MacroKind::ChNoun => "ChNoun",
        MacroKind::LowerHead => "LowerHead",
        MacroKind::ColorType => "ColorType",
        MacroKind::EdgeColorType => "EdgeColorType",
        MacroKind::Ruby => "Ruby",
        MacroKind::Digit => "Digit",
        MacroKind::Ordinal => "Ordinal",
        MacroKind::Sound => "Sound",
        MacroKind::LevelPos => "LevelPos",
        MacroKind::Unknown(value) => return Cow::Owned(format!("Unknown({value})")),
        _ => return Cow::Owned(format!("{kind:?}")),
    };

    Cow::Borrowed(name)
}
