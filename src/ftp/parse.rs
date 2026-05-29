use itertools::Itertools;
use nom::bytes::{tag, take_until};
use nom::character::streaming::{digit1, multispace0, not_line_ending};
use nom::combinator::map_res;
use nom::multi::separated_list1;
use nom::sequence::{delimited, preceded};
use nom::{IResult, Parser};

/// On parsing a code 227, this will parse the remaining input into
/// an address usable by a socket
pub fn passive_mode_ip_address(input: &str) -> IResult<&str, String> {
    let (input, _) = take_until("(").parse(input)?;

    let parser = delimited(
        tag("("),
        separated_list1(tag(","), nom::character::streaming::u32),
        tag(")"),
    );

    let mut parser = map_res(parser, |numbers| {
        let host: String = Itertools::join(&mut numbers[0..=3].iter(), ".");
        Result::<String, ()>::Ok(format!("{}:{}", host, (numbers[4] << 8) + numbers[5]))
    });

    parser.parse(input)
}

/// Dedicated function for parsing responses from the server
pub fn response(input: &str) -> IResult<&str, u32> {
    let (rest, num) = digit1(input)?;
    let mut parser = preceded(multispace0, not_line_ending);
    let (_, rest) = parser.parse(rest)?;
    Ok((
        rest,
        num.parse().expect("Should be a 3 digit response code"),
    ))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_passive_mode() {
        let result = response("227 Entering Passive Mode (192,168,150,90,195,149).\r\n");
        assert_eq!(
            result,
            Ok(("Entering Passive Mode (192,168,150,90,195,149).", 227))
        );
    }

    #[test]
    fn test_passive_response_to_ip() {
        let result = passive_mode_ip_address("Entering Passive Mode (192,168,150,90,195,149).");
        assert_eq!(result, Ok((".", "192.168.150.90:50069".to_string())));
    }
}
