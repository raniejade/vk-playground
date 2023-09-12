use runtime::{App, AppContext};

struct MyApp;

impl App for MyApp {
    fn get_title(&mut self) -> anyhow::Result<String> {
        Ok(String::from("Triangle"))
    }

    fn frame(&mut self, ctx: &mut AppContext) -> anyhow::Result<()> {
        // let idx = ctx.acquire_next_image_from_swapchain(u64::MAX, None, None)?;
        Ok(())
    }
}

fn main() {
    let app = MyApp {};
    runtime::run(app).unwrap();
}
