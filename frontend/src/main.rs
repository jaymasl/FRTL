use frontend::App;
use wasm_logger;
use yew::Renderer;

fn main() {
    // Initialize the logger for WebAssembly
    wasm_logger::init(wasm_logger::Config::default());

    Renderer::<App>::new().render();
}