// use api_types::Lobby;
// use dioxus::prelude::*;

// #[component]
// pub fn LobbyComponent(lobby: Lobby, join_lobby: EventHandler) -> Element {
//     rsx!(
//         tr { key: "{lobby.name}",
//             td { class: "px-6 py-4 whitespace-nowrap", "{lobby.name}" }
//             td { class: "px-6 py-4 whitespace-nowrap", "{lobby.players.len()}/{lobby.max_players}" }
//             td { class: "px-6 py-4 whitespace-nowrap", "{lobby.game_mode}" }
//             td { class: "px-6 py-4 whitespace-nowrap",
//                 // button {
//                 //     onclick: move |evt| join_lobby.call(lobby.clone()),
//                 //     disabled: true,
//                 //     class: "px-4 py-2 rounded-md text-sm font-medium bg-yellow-300",
//                 //     "Join lobby"
//                 // }
//                 Link {
//                     to: AppRoutes::GameRoom {
//                         room_code: lobby.name.clone(),
//                     },
//                     onclick: move |evt| { app_props.write().lobby_code = lobby.name.clone() },
//                     class: "px-4 py-2 rounded-md text-sm font-medium bg-yellow-300",
//                     "Join lobby"
//                 }
//             }
//         }
//     )
// }

// #[component]
// pub fn LobbyList(
//     lobbies: Vec<Lobby>,
//     refresh_lobbies: EventHandler,
//     join_lobby: EventHandler,
// ) -> Element {
//     let lobby = String::from("test");
//     rsx!(
//         div { class: "container mx-auto p-4",
//             div { class: "flex flex-row justify-center gap-2",
//                 h1 { class: "text-2xl font-bold mb-4", "Game Lobbies" }
//                 button {
//                     class: "bg-gray-300 flex flex-row text-center border p-1 border-solid border-black rounded-md justify-center items-center",
//                     onclick: move |evt| refresh_lobbies.call(()),
//                     svg {
//                         class: "w-6 h-6",
//                         fill: "none",
//                         stroke: "currentColor",
//                         "stroke-width": "1",
//                         "view-box": "0 0 24 24",
//                         path {
//                             "stroke-linecap": "round",
//                             "stroke-linejoin": "round",
//                             d: "M4 4v5h.582c.523-1.838 1.856-3.309 3.628-4.062A7.978 7.978 0 0112 4c4.418 0 8 3.582 8 8s-3.582 8-8 8a7.978 7.978 0 01-7.658-5.125c-.149-.348-.54-.497-.878-.365s-.507.537-.355.885A9.956 9.956 0 0012 22c5.523 0 10-4.477 10-10S17.523 2 12 2c-2.045 0-3.94.613-5.514 1.653A6.978 6.978 0 004.582 4H4z"
//                         }
//                     }
//                     label { class: "text-lg", "Refresh" }
//                 }
//             }
//             div { class: "relative mb-4",
//                 svg {
//                     class: "absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 h-5 w-5",
//                     "xmlns": "http://www.w3.org/2000/svg",
//                     height: "24",
//                     "stroke-linejoin": "round",
//                     "viewBox": "0 0 24 24",
//                     "stroke-width": "2",
//                     "fill": "none",
//                     "stroke-linecap": "round",
//                     "stroke": "currentColor",
//                     width: "24",
//                     class: "lucide lucide-search",
//                     circle { "r": "8", "cx": "11", "cy": "11" }
//                     path { "d": "m21 21-4.3-4.3" }
//                 }
//                 input {
//                     r#type: "text",
//                     placeholder: "Search lobbies...",
//                     value: "searchTerm",
//                     // onChange: move || {},
//                     class: "w-full pl-10 pr-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
//                 }
//                 div { class: "overflow-x-auto",
//                     table { class: "min-w-full bg-white border border-gray-300",
//                         thead {
//                             tr { class: "bg-gray-100",
//                                 th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
//                                     "Lobby Name"
//                                 }
//                                 th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
//                                     "Players"
//                                 }
//                                 th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
//                                     "Game Mode"
//                                 }
//                                 th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
//                                     "Action"
//                                 }
//                             }
//                         }
//                         tbody { class: "divide-y divide-gray-200",
//                             "filteredlobbies"

//                             {lobbies.iter().map(|lobby| {
//                                 rsx!(
//                                     LobbyComponent {
//                                         lobby: lobby.clone(),
//                                         join_lobby: join_lobby,
//                                     }
//                                 )
//                             })
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//     )
// }
