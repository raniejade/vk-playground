use runtime::{App, RuntimeContext};

struct MyApp;

impl App for MyApp {
    fn get_title(&mut self) -> anyhow::Result<String> {
        Ok(String::from("Triangle"))
    }

    fn frame(&mut self, ctx: &mut RuntimeContext) -> anyhow::Result<()> {
        Ok(())
    }
}

fn main() {
    let app = MyApp {};
    runtime::run(app).unwrap();
}
