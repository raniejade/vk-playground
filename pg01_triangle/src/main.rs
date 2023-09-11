use runtime::{App, AppContext};

struct MyApp;

impl App for MyApp {
    fn get_title(&mut self) -> anyhow::Result<String> {
        Ok(String::from("Triangle"))
    }

    fn frame(&mut self, ctx: &mut AppContext) -> anyhow::Result<()> {
        Ok(())
    }
}

fn main() {
    let app = MyApp {};
    runtime::run(app).unwrap();
}
