#[macro_export]
macro_rules! log {
    ($s:literal) => {{
        print!($s);
        stdout().flush()?;
    }};
}

#[macro_export]
macro_rules! y_n_reverse {
    ($fmt:literal, $bool:expr) => {{
        if $bool {
            println!("{}: {}", $fmt, "yes".red());
        } else {
            println!("{}: {}", $fmt, "no".green());
        }
    }};
}

#[macro_export]
macro_rules! status {
    ($code:expr) => {{
        let ret = $code;
        match &ret {
            Ok(_) => println!("{}", "ok".green()),
            Err(_) => println!("{}", "failed".red()),
        }
        ret
    }};
}
