use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct GradientBackgroundProps {
    #[prop_or_default]
    pub children: Html,
}

/// A reusable gradient background component that provides the same visual style
/// across all pages of the application.
#[function_component(GradientBackground)]
pub fn gradient_background(props: &GradientBackgroundProps) -> Html {
    html! {
        <div class="relative min-h-screen">
            // Base color - light for light mode, dark for dark mode
            <div class="fixed inset-0 bg-white dark:bg-gray-950 -z-50"></div>
            
            // Content container
            <div class="relative z-0">
                {props.children.clone()}
            </div>
        </div>
    }
}