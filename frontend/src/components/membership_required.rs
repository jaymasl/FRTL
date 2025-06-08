use yew::prelude::*;
use crate::styles;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub feature_name: String,
}

#[function_component(MembershipRequired)]
pub fn membership_required(props: &Props) -> Html {
    html! {
        <div class="flex flex-col items-center justify-center p-6 bg-gray-50 dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 max-w-md mx-auto">
            <div class="text-center mb-4">
                <span class="inline-flex items-center px-3 py-1 rounded-full text-sm font-medium bg-amber-100 text-amber-800 dark:bg-amber-800 dark:text-amber-100">
                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 mr-1.5" viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clip-rule="evenodd" />
                    </svg>
                    {"Members Only Feature"}
                </span>
            </div>
            
            <h3 class="text-xl font-bold mb-2 text-gray-900 dark:text-white">
                {format!("{} requires membership", props.feature_name)}
            </h3>
            
            <p class="text-gray-600 dark:text-gray-300 mb-6 text-center">
                {"This feature is available exclusively to members. Activate a membership code to unlock this and other premium features."}
            </p>
            
            <a href="/settings" class={styles::BUTTON_PRIMARY}>
                {"Activate Membership"}
            </a>
        </div>
    }
} 