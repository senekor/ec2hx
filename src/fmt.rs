/// This is used to support the key `trim_trailing_whitespace` for languages
/// that don't already have a formatter.
pub fn trim_trailing_whitespace(input: &str) -> String {
    let end_of_line = if input.contains("\r\n") {
        "\r\n"
    } else if input.contains("\n") {
        "\n"
    } else {
        // input doesn't contain a line break. Let's not make assumptions and
        // wait for user to a at least one line before formatting.
        return input.into();
    };
    let mut buf = String::new();
    for line in input.lines() {
        buf.push_str(line.trim_end());
        buf.push_str(end_of_line);
    }
    if input.as_bytes().last() != Some(&b'\n') {
        // We added one too many newlines at the end.
        buf.pop();
        if buf.as_bytes().last() == Some(&b'\r') {
            buf.pop();
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    #[test]
    fn trim_trailing_whitespace() {
        let input = "
normal line
the next line has four spaces
    
the next line has a tab character
	
this line has trailing spaces    
        ";
        let expected = "
normal line
the next line has four spaces

the next line has a tab character

this line has trailing spaces
";
        let actual = super::trim_trailing_whitespace(input);
        assert_eq!(actual, expected);
    }
}
