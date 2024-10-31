use dioxus::prelude::*;

#[component]
pub fn LobbyList() -> Element {
    let lobby = String::from("test");
    rsx!(
        div { class: "container mx-auto p-4",
            h1 { class: "text-2xl font-bold mb-4", "Game Lobbies" }
            div { class: "relative mb-4",
                svg {
                    class: "absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 h-5 w-5",
                    "xmlns": "http://www.w3.org/2000/svg",
                    height: "24",
                    "stroke-linejoin": "round",
                    "viewBox": "0 0 24 24",
                    "stroke-width": "2",
                    "fill": "none",
                    "stroke-linecap": "round",
                    "stroke": "currentColor",
                    width: "24",
                    class: "lucide lucide-search",
                    circle { "r": "8", "cx": "11", "cy": "11" }
                    path { "d": "m21 21-4.3-4.3" }
                }
                input {
                    r#type: "text",
                    placeholder: "Search lobbies...",
                    value: "searchTerm",
                    // onChange: move || {},
                    class: "w-full pl-10 pr-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                }
                div { class: "overflow-x-auto",
                    table { class: "min-w-full bg-white border border-gray-300",
                        thead {
                            tr { class: "bg-gray-100",
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Lobby Name"
                                }
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Players"
                                }
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Game Mode"
                                }
                                th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider",
                                    "Action"
                                }
                            }
                        }
                        tbody { class: "divide-y divide-gray-200",
                            "filteredlobbies"
                            tr { key: "{lobby}",
                                td { class: "px-6 py-4 whitespace-nowrap", "lobby.name" }
                                td { class: "px-6 py-4 whitespace-nowrap",
                                    "players in lobby/allowed players"
                                }
                                td { class: "px-6 py-4 whitespace-nowrap", "lobby.gameMode" }
                                td { class: "px-6 py-4 whitespace-nowrap",
                                    button {
                                        // onclick: move || {},
                                        disabled: true,
                                        class: "px-4 py-2 rounded-md text-sm font-medium",
                                        "lobby.players >= lobby.maxPlayers"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    )
}
