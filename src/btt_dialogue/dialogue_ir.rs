use std::error::Error;

use ironworks::sestring::{Expression, MacroKind, MacroPayload, Payload, SeString};

use super::binary::{StringPool, checked_u32, write_u8, write_u32};
use super::contract::{
    IR_IF, IR_PARAMETER, IR_PLACEHOLDER, IR_PLAYER_NAME, IR_SEQUENCE, IR_SWITCH, IR_TEXT,
};

// payload and macro classification
// --------------------------------
pub(super) fn encode_dialogue_ir(
    value: &SeString,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    let payloads = value.payloads().collect::<Result<Vec<_>, _>>()?;
    write_sequence_header(output, payloads.len())?;
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
        Payload::Text(text) => encode_text(text.as_utf8()?, output, pool),
        Payload::Macro(payload) => encode_macro(payload, output, pool),
    }
}

fn encode_macro(
    payload: MacroPayload,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    let args = payload.expressions().collect::<Result<Vec<_>, _>>()?;
    match payload.kind() {
        // Literal text equivalents.
        MacroKind::NewLine => encode_text("\n", output, pool),
        MacroKind::NonBreakingSpace => encode_text("\u{00a0}", output, pool),
        MacroKind::SoftHyphen => encode_placeholder("softHyphen", output, pool),
        MacroKind::Hyphen => encode_text("–", output, pool),

        // Runtime values the final plugin can still fill in.
        MacroKind::PcName => encode_player_name(output),

        // Control flow kept as IR for JS policy and runtime template handling.
        MacroKind::If => encode_if_macro("If", &args, output, pool),
        MacroKind::IfPcGender => encode_named_if("IfPcGender", &args, output, pool),
        MacroKind::IfPcName => encode_named_if("IfPcName", &args, output, pool),
        MacroKind::IfSelf => encode_named_if("IfSelf", &args, output, pool),
        MacroKind::Switch => encode_switch_macro(&args, output, pool),

        // Macros whose useful text is carried by their arguments.
        MacroKind::Ruby => encode_first_arg(&args, output, pool),
        MacroKind::String => encode_expression_sequence(&args, output, pool),
        MacroKind::Split => encode_split(&args, output, pool),
        MacroKind::Head
        | MacroKind::HeadAll
        | MacroKind::Lower
        | MacroKind::LowerHead
        | MacroKind::Caps
        | MacroKind::Fixed => encode_first_textual_arg(&args, output, pool),
        MacroKind::JaNoun
        | MacroKind::EnNoun
        | MacroKind::DeNoun
        | MacroKind::FrNoun
        | MacroKind::ChNoun => encode_noun(&args, output, pool),

        // Semantic placeholders that affect matching but are not rendered now.
        MacroKind::Sheet => encode_placeholder("sheet", output, pool),
        MacroKind::Icon | MacroKind::Icon2 => encode_placeholder("icon", output, pool),
        MacroKind::SetTime | MacroKind::SetResetTime | MacroKind::Time => {
            encode_placeholder("time", output, pool)
        }
        MacroKind::Num
        | MacroKind::Kilo
        | MacroKind::Byte
        | MacroKind::Sec
        | MacroKind::Hex
        | MacroKind::Float
        | MacroKind::Digit
        | MacroKind::Ordinal
        | MacroKind::LevelPos
        | MacroKind::Key
        | MacroKind::Josa
        | MacroKind::Josaro => encode_placeholder("value", output, pool),

        // Pure presentation/control macros do not contribute dialogue text.
        MacroKind::Italic
        | MacroKind::Bold
        | MacroKind::Edge
        | MacroKind::Shadow
        | MacroKind::Link
        | MacroKind::Scale
        | MacroKind::Wait
        | MacroKind::Sound
        | MacroKind::Color
        | MacroKind::ColorType
        | MacroKind::EdgeColor
        | MacroKind::EdgeColorType
        | MacroKind::ShadowColor => encode_empty_sequence(output),

        other => Err(format!("Unsupported BTT dialogue macro kind: {other:?}").into()),
    }
}

// IR node writers
// --------------------------------
fn write_sequence_header(output: &mut Vec<u8>, count: usize) -> Result<(), Box<dyn Error>> {
    write_u8(output, IR_SEQUENCE);
    write_u32(output, checked_u32(count, "IR sequence item count")?);
    Ok(())
}

fn encode_text(
    text: &str,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    write_u8(output, IR_TEXT);
    write_u32(output, pool.add(text)?);
    Ok(())
}

fn encode_placeholder(
    kind: &str,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    write_u8(output, IR_PLACEHOLDER);
    write_u32(output, pool.add(kind)?);
    Ok(())
}

fn encode_parameter(
    kind: &str,
    index: &Expression,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    write_u8(output, IR_PARAMETER);
    write_u32(output, pool.add(kind)?);
    write_u32(output, pool.add(&expression_name(index)?)?);
    Ok(())
}

fn encode_player_name(output: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
    write_u8(output, IR_PLAYER_NAME);
    Ok(())
}

fn encode_empty_sequence(output: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
    write_sequence_header(output, 0)
}

// macro argument compilation
// --------------------------------
fn encode_if_macro(
    name: &str,
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    if args.len() < 2 {
        return Err(format!("{name} requires at least two arguments").into());
    }

    write_u8(output, IR_IF);
    write_u32(output, pool.add(&expression_name(&args[0])?)?);
    encode_text_argument(&args[1], output, pool)?;
    if let Some(fallback) = args.get(2) {
        encode_text_argument(fallback, output, pool)
    } else {
        encode_empty_sequence(output)
    }
}

fn encode_named_if(
    condition: &str,
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err(format!("{condition} requires at least one argument").into());
    }

    write_u8(output, IR_IF);
    write_u32(output, pool.add(condition)?);
    encode_text_argument(&args[0], output, pool)?;
    if let Some(fallback) = args.get(1) {
        encode_text_argument(fallback, output, pool)
    } else {
        encode_empty_sequence(output)
    }
}

fn encode_switch_macro(
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        return Err("Switch requires a condition argument".into());
    }

    write_u8(output, IR_SWITCH);
    write_u32(output, pool.add(&expression_name(&args[0])?)?);
    write_u32(output, checked_u32(args.len() - 1, "Switch case count")?);
    for arg in args.iter().skip(1) {
        encode_text_argument(arg, output, pool)?;
    }
    Ok(())
}

fn encode_first_arg(
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    if let Some(arg) = args.first() {
        encode_text_argument(arg, output, pool)
    } else {
        encode_empty_sequence(output)
    }
}

fn encode_expression_sequence(
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    if args.len() == 1 {
        return encode_expression_node(&args[0], output, pool);
    }

    write_sequence_header(output, args.len())?;
    for arg in args {
        encode_expression_node(arg, output, pool)?;
    }
    Ok(())
}

fn encode_split(
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    if let Some(arg) = args.first() {
        encode_expression_node(arg, output, pool)
    } else {
        encode_placeholder("value", output, pool)
    }
}

fn encode_first_textual_arg(
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    for arg in args {
        let mut candidate = Vec::new();
        encode_text_argument(arg, &mut candidate, pool)?;
        if !is_empty_ir(&candidate) {
            output.extend_from_slice(&candidate);
            return Ok(());
        }
    }

    encode_empty_sequence(output)
}

fn encode_noun(
    args: &[Expression],
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    let sheet = args.first().and_then(literal_sestring_argument);
    let row = args.get(2).map(expression_name).transpose()?;
    if sheet.as_deref().is_some_and(|value| !value.is_empty())
        && row.as_deref().is_some_and(|value| !value.is_empty())
    {
        return encode_placeholder("sheet", output, pool);
    }

    encode_first_textual_arg(args, output, pool)
}

fn encode_text_argument(
    expression: &Expression,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    encode_expression_node(expression, output, pool)
}

// expression rendering
// --------------------------------
fn encode_expression_node(
    expression: &Expression,
    output: &mut Vec<u8>,
    pool: &mut StringPool,
) -> Result<(), Box<dyn Error>> {
    match expression {
        Expression::SeString(value) => encode_dialogue_ir(value, output, pool),
        Expression::LocalNumber(value) => encode_parameter("integer", value, output, pool),
        Expression::GlobalNumber(value) => encode_parameter("player", value, output, pool),
        Expression::LocalString(value) => encode_parameter("string", value, output, pool),
        Expression::GlobalString(value) => encode_parameter("object", value, output, pool),
        _ => encode_text(&expression_name(expression)?, output, pool),
    }
}

fn expression_name(expression: &Expression) -> Result<String, Box<dyn Error>> {
    Ok(match expression {
        Expression::U32(value) => value.to_string(),
        Expression::SeString(value) => literal_sestring_argument(expression)
            .unwrap_or_else(|| format!("<sestring:{}>", value.payloads().count())),
        Expression::Millisecond => "Millisecond".to_string(),
        Expression::Second => "Second".to_string(),
        Expression::Minute => "Minute".to_string(),
        Expression::Hour => "Hour".to_string(),
        Expression::Day => "Day".to_string(),
        Expression::Weekday => "Weekday".to_string(),
        Expression::Month => "Month".to_string(),
        Expression::Year => "Year".to_string(),
        Expression::StackColor => "StackColor".to_string(),
        Expression::LocalNumber(value) => format!("IntegerParameter({})", expression_name(value)?),
        Expression::GlobalNumber(value) => format!("PlayerParameter({})", expression_name(value)?),
        Expression::LocalString(value) => format!("StringParameter({})", expression_name(value)?),
        Expression::GlobalString(value) => format!("ObjectParameter({})", expression_name(value)?),
        Expression::Ge(left, right) => binary_expression_name("GreaterThanOrEqualTo", left, right)?,
        Expression::Gt(left, right) => binary_expression_name("GreaterThan", left, right)?,
        Expression::Le(left, right) => binary_expression_name("LessThanOrEqualTo", left, right)?,
        Expression::Lt(left, right) => binary_expression_name("LessThan", left, right)?,
        Expression::Eq(left, right) => binary_expression_name("Equal", left, right)?,
        Expression::Ne(left, right) => binary_expression_name("NotEqual", left, right)?,
        Expression::Unknown(value) => {
            return Err(format!("Unknown BTT dialogue expression code: {value}").into());
        }
        other => return Err(format!("Unhandled BTT dialogue expression: {other:?}").into()),
    })
}

fn binary_expression_name(
    name: &str,
    left: &Expression,
    right: &Expression,
) -> Result<String, Box<dyn Error>> {
    Ok(format!(
        "{name}({},{})",
        expression_name(left)?,
        expression_name(right)?
    ))
}

fn literal_sestring_argument(expression: &Expression) -> Option<String> {
    let Expression::SeString(value) = expression else {
        return None;
    };

    let mut output = String::new();
    for payload in value.payloads() {
        let Ok(Payload::Text(text)) = payload else {
            return None;
        };
        let Ok(text) = text.as_utf8() else {
            return None;
        };
        output.push_str(text);
    }

    Some(output.trim().to_string())
}

fn is_empty_ir(bytes: &[u8]) -> bool {
    bytes == [IR_SEQUENCE, 0, 0, 0, 0]
}
