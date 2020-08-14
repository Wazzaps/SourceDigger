let query_form = document.getElementById("query-form");
let query_input = document.getElementById("query");
let results_frame = document.getElementById("results");
let autocomplete_container = document.getElementById("autocomplete");
let autocomplete_items = document.getElementById("autocomplete-results");

function update_query(changes) {
    if (!changes) {
        return;
    }
    let current_url = new URL(location.href);
    let current_params = current_url.searchParams;
    let has_changes = false;
    for(let c in changes) {
        if (current_params.get(c) !== changes[c]) {
            has_changes = true;
            current_params.set(c, changes[c]);
        }
    }
    let project_name = location.pathname.split("/")[1];
    project_name = project_name[0].toUpperCase() + project_name.substring(1);

    if (current_params.get("q")) {
        document.title = current_params.get("q") + " - " + project_name + " - SourceDigger";
    } else {
        document.title = project_name + " - SourceDigger";
    }
    if (has_changes) {
        history.pushState(null, document.title, current_url.toString());
        do_update_query(current_url);
    }
}

function do_update_query(url) {
    query_input.value = url.searchParams.get("q");
    url.pathname += "/diffs";
    if (results_frame.contentWindow.location.href !== url.href) {
        results_frame.contentWindow.location.replace(url.href);
    }
    try_autocomplete();
}

addEventListener("popstate", () => {
    do_update_query(new URL(location.href));
})

query_form.addEventListener("submit", (e) => {
    update_query({"q": query_input.value});
    e.preventDefault();
    return false;
});

results_frame.addEventListener("load", () => {
    results_frame.contentDocument.body.addEventListener("click", (e) => {
        let parent = e.target.parentNode;
        let elem = e.target;
        if (parent.tagName !== "DIV") {
            elem = parent;
            parent = parent.parentNode;
        }
        let i = 0;
        for (let child of parent.childNodes) {
            if (child === elem) {
                break;
            }
            i++;
        }
        if (i === 0 && elem.className) {
            update_query({"t": elem.className});
        } else if (i === 1 && elem.className) {
            update_query({"a": elem.className});
        } else if (i === 2) {
            update_query({"q": new URL(elem.href).searchParams.get("q")});
        } else if (i === 4) {
            location.href = elem.href;
        }

        e.preventDefault();
        return false;
    });
});

let timer = null;
let last_autocomplete_query = "";

async function fetch_autocomplete(query) {
    console.log(query);
    let url = new URL(location.href);
    url.pathname += "/autocomplete";
    url.searchParams.set("q", query + ".*");
    // console.log(url);
    let results = (await (await fetch(url.toString())).text()).split("\n");
    let links = [];
    let i = 2;
    for (let result of results) {
        if (!result) {
            continue;
        }
        links.push("<li><a href='#' tabindex='" + i + "' onclick='handle_ac_link(this)'>" + result + "</a></li>");
        i++;
    }
    autocomplete_items.innerHTML = links.join("");
    autocomplete_items.scrollTo(0, 0);
    autocomplete_items.style.display = links.length ? "block" : "none";
    // console.log(results);
}

function try_autocomplete() {
    if (timer == null) {
        if (query_input.value !== last_autocomplete_query) {
            last_autocomplete_query = query_input.value;
            fetch_autocomplete(query_input.value);
            timer = setInterval(() => {
                if (query_input.value !== last_autocomplete_query) {
                    console.log(last_autocomplete_query, "->", query_input.value);
                    last_autocomplete_query = query_input.value;
                    fetch_autocomplete(query_input.value);
                } else {
                    clearInterval(timer);
                    timer = null;
                }
            }, 150);
        }
    }
}

query_input.addEventListener("keyup", () => {
    try_autocomplete()
});

function handle_ac_link(e) {
    update_query({"q": e.innerText});
}

// query_input.addEventListener("focusin", () => {
//     autocomplete_container.style.display = "block";
// });
//
// query_input.addEventListener("focusout", () => {
//     // if (!autocomplete_container.contains(document.activeElement)) {
//     //     autocomplete_container.style.display = "none";
//     // }
// });
//
// autocomplete_container.addEventListener("focusin", () => {
//     console.log(".");
//     autocomplete_container.style.display = "block";
// });
//
// autocomplete_container.addEventListener("focusout", () => {
//     console.log("!");
//     autocomplete_container.style.display = "none";
// });