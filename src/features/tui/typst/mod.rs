pub mod markup;
pub mod renderer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::ast::Stmt;
    use crate::features::core::evaluate;
    use crate::features::core::notation::{ExponentStep, Notation, NumberFormat};
    use crate::features::core::parser::parse_stmt;
    use crate::features::core::{Answer, Environment, Evaluation, RenderMode};
    use crate::features::tui::theme::Theme;

    fn render(src: String) -> image::RgbaImage {
        renderer::TypstRenderer::new(src)
            .render(2.0)
            .unwrap_or_else(|e| panic!("render failed: {e}"))
    }

    #[test]
    fn all_constructs_compile() {
        let theme = Theme::cyberpunk();
        let inputs = [
            "1+2",
            "x-y",
            "-x",
            "--x",
            "2*3",
            "3y",
            "2(x+1)",
            "(a+b)*(c-d)",
            "a/(b+c)",
            "1/4",
            "x^2",
            "(x+1)^(1/2)",
            "2^-3",
            "sin(x)",
            "cos(pi/2)",
            "sqrt(2)",
            "frac(1,2)",
            "abs(-3)",
            "foo(1,2,3)",
            "clear()",
            "delvar(x)",
            "delvar(physics, -r)",
            "physics.a1.g * 2",
            "set(notation, si)",
            "9.81_m/s^2",
            "(a+b)_km",
            "72_km/h -> m/s",
            "5_µs + 2_kOhm",
            "e",
            "pi*r^2",
            "my_var + 1.5",
            "x := 3y = 2",
            "3y = 2",
            "0.000000001",
            "1e",
            "exp(1)",
            "ln(2)",
            "logb(2, 8)",
            "logb(b, x)",
            "root(3, 8)",
            "root(n, x)",
        ];
        for src in inputs {
            let Ok(stmt) = parse_stmt(src) else { continue };
            render(markup::input_source(&stmt, &theme));
        }

        for mode in [RenderMode::Decimal, RenderMode::Symbolic] {
            let mut env = Environment::new();
            for src in [
                "x := 0.25",
                "x + 1/3",
                "2x + 3x",
                "sqrt(2)",
                "pi/2",
                "2^100",
                "3y = 2",
                "1/3 * y",
                "-1/3 * y",
                "y^2/4",
                "0.1 + 0.2",
                "2^-40",
                "1/10^20",
                "(a+1)/2",
            ] {
                if let Evaluation::Answer(a @ Answer::Value { .. }) = evaluate(&mut env, src, mode)
                {
                    render(markup::answer_source(&a, &theme, &NumberFormat::default()).unwrap());
                }
            }
        }
    }

    #[test]
    fn every_number_format_compiles() {
        let theme = Theme::cyberpunk();
        let mut env = Environment::new();
        let answers: Vec<Answer> = [
            "1500",
            "-0.0000047",
            "2^100",
            "1/3",
            "12y",
            "10^40",
            "1500_m",
            "2_N*3_m",
            "72_km/h -> m/s",
            "1/(2_s)",
            "3_kOhm",
            "x*2_m",
            "x + 1_m",
        ]
        .iter()
        .filter_map(|src| match evaluate(&mut env, src, RenderMode::Decimal) {
            Evaluation::Answer(a @ Answer::Value { .. }) => Some(a),
            _ => None,
        })
        .collect();
        for notation in Notation::ALL {
            for exponent_step in ExponentStep::ALL {
                for decimal_places in [None, Some(2)] {
                    let numbers = NumberFormat {
                        decimal_places,
                        notation,
                        exponent_step,
                        extended_prefixes: true,
                    };
                    for a in &answers {
                        render(markup::answer_source(a, &theme, &numbers).unwrap());
                    }
                }
            }
        }
    }

    fn dominant(img: &image::RgbaImage) -> (u8, u8, u8) {
        let mut best = (0u32, (0u8, 0u8, 0u8));
        let mut counts = std::collections::HashMap::new();
        for p in img.pixels().filter(|p| p[3] > 200) {
            let c = counts.entry((p[0], p[1], p[2])).or_insert(0u32);
            *c += 1;
            if *c > best.0 {
                best = (*c, (p[0], p[1], p[2]));
            }
        }
        best.1
    }

    fn assert_close(got: (u8, u8, u8), want: (u8, u8, u8)) {
        let close = |a: u8, b: u8| a.abs_diff(b) <= 2;
        assert!(
            close(got.0, want.0) && close(got.1, want.1) && close(got.2, want.2),
            "typeset colour {got:?} differs from terminal colour {want:?}"
        );
    }

    #[test]
    fn typeset_colours_match_terminal_colours() {
        use crate::features::tui::theme::{LineRole, TokenKind, rgb};
        let theme = Theme::cyberpunk();

        let mut env = Environment::new();
        let Evaluation::Answer(a) = evaluate(&mut env, "123456", RenderMode::Decimal) else {
            panic!()
        };
        let img = render(markup::answer_source(&a, &theme, &NumberFormat::default()).unwrap());
        assert_close(
            dominant(&img),
            rgb(theme.token_color(LineRole::Result, TokenKind::Number)),
        );

        let stmt = parse_stmt("123456").unwrap();
        let Stmt::Expr(_) = &stmt else { panic!() };
        let img = render(markup::input_source(&stmt, &theme));
        assert_close(dominant(&img), rgb(theme.dim));

        assert!(img.pixels().any(|p| p[3] == 0));
    }

    #[test]
    fn transparent_background_and_sane_size() {
        let theme = Theme::cyberpunk();
        let stmt = parse_stmt("1+1").unwrap();
        let img = render(markup::input_source(&stmt, &theme));
        assert!(img.width() > 10 && img.height() > 10);
        assert!(
            img.height() < 80,
            "single-line math should be short: {}",
            img.height()
        );
    }
}
