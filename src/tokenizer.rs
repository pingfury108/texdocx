#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Text(String),
    InlineFormula(String),
    DisplayFormula(String),
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '$' {
            let start = i;
            while i < chars.len() && chars[i] != '$' {
                i += 1;
            }
            tokens.push(Token::Text(chars[start..i].iter().collect()));
        } else if i + 1 < chars.len() && chars[i + 1] == '$' {
            i += 2;
            let start = i;
            while i + 1 < chars.len() && !(chars[i] == '$' && chars[i + 1] == '$') {
                i += 1;
            }
            let formula: String = chars[start..i].iter().collect();
            if formula.trim().len() > 0 {
                tokens.push(Token::DisplayFormula(formula.trim().to_string()));
            }
            i += 2;
        } else {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '$' {
                i += 1;
            }
            let formula: String = chars[start..i].iter().collect();
            if formula.trim().len() > 0 {
                tokens.push(Token::InlineFormula(formula.trim().to_string()));
            }
            i += 1;
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let tokens = tokenize("Hello World");
        assert_eq!(tokens, vec![Token::Text("Hello World".to_string())]);
    }

    #[test]
    fn test_inline_formula() {
        let tokens = tokenize("a $x^2$ b");
        assert_eq!(
            tokens,
            vec![
                Token::Text("a ".to_string()),
                Token::InlineFormula("x^2".to_string()),
                Token::Text(" b".to_string()),
            ]
        );
    }

    #[test]
    fn test_display_formula() {
        let tokens = tokenize("text $$E=mc^2$$ more");
        assert_eq!(
            tokens,
            vec![
                Token::Text("text ".to_string()),
                Token::DisplayFormula("E=mc^2".to_string()),
                Token::Text(" more".to_string()),
            ]
        );
    }

    #[test]
    fn test_mixed() {
        let input = "在$\\triangle ABC$中，$$a^2+b^2=c^2$$证明完毕。";
        let tokens = tokenize(input);
        assert_eq!(tokens.len(), 5);
        assert_eq!(
            tokens,
            vec![
                Token::Text("在".to_string()),
                Token::InlineFormula("\\triangle ABC".to_string()),
                Token::Text("中，".to_string()),
                Token::DisplayFormula("a^2+b^2=c^2".to_string()),
                Token::Text("证明完毕。".to_string()),
            ]
        );
    }
}
