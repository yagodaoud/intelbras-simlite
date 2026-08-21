fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/simlite.ico");
        if let Err(err) = res.compile() {
            // Ícone do .exe é opcional; a UI ainda usa o PNG do Slint.
            eprintln!("aviso: não foi possível embutir ícone no .exe: {err}");
        }
    }

    slint_build::compile("ui/app.slint").expect("compile Slint UI");
}
