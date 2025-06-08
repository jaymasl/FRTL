use crate::pages::games::{
    Games,
    frontend_snake_game::FrontendSnakeGame,
    frontend_2048_game::Frontend2048Game,
    frontend_word_game::FrontendWordGame,
    frontend_hexort_game::FrontendHexortGame,
};
use crate::pages::match_game::FrontendMatchGame;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/games")]
    Games,
    #[at("/games/snake")]
    Snake,
    #[at("/games/2048")]
    Game2048,
    #[at("/games/word")]
    WordGame,
    #[at("/games/hexort")]
    Hexort,
    #[at("/games/match")]
    Match,
    #[not_found]
    #[at("/404")]
    NotFound,
    // ... other routes ...
}

pub fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home /> },
        Route::Games => html! { <Games /> },
        Route::Snake => html! { <FrontendSnakeGame /> },
        Route::Game2048 => html! { <Frontend2048Game /> },
        Route::WordGame => html! { <FrontendWordGame /> },
        Route::Hexort => html! { <FrontendHexortGame /> },
        Route::Match => html! { <FrontendMatchGame /> },
        Route::NotFound => html! { <NotFound /> },
        // ... other route matches ...
    }
} 