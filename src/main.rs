use playwright::Playwright;
use playwright::api::Page;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FormField {
    name: String,
    value: String,
    #[serde(rename = "type")] // 'type' is a keyword in JS/attribute, map it
    field_type: String,
}

#[derive(Debug, Deserialize)]
pub struct BookLink {
    slug: String,
    title: String,
}

/// Verify that the login succeeded by navigating to the published books page
/// and checking both the final URL and the page title. Returns true on success.
pub async fn verify_login(page: &Page) -> Result<bool, playwright::Error> {
    const PUBLISHED_URL: &str = "https://leanpub.com/author_dashboard/books/published";
    if let Err(e) = page.goto_builder(PUBLISHED_URL).goto().await {
        eprintln!("Navigation to published books page failed: {}", e);
        return Ok(false);
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await; // allow dynamic content to load
    let final_url: String = page.eval("() => location.href").await.unwrap_or_default();
    let published_title: String = page.eval("() => document.title").await.unwrap_or_default();
    println!("Final URL: {}", final_url);
    println!("Final Title: {}", published_title);
    let success = final_url == PUBLISHED_URL && published_title.trim() == "Leanpub - Your Books";
    if success {
        println!("Login success verified: reached published books page.");
    } else {
        eprintln!(
            "Login verification failed: expected URL {} with title 'Leanpub - Your Books'.",
            PUBLISHED_URL
        );
    }
    Ok(success)
}

/// After a successful login (and while on the published books page) collect slug/title pairs.
pub async fn fetch_published_books(page: &Page) -> Result<Vec<BookLink>, playwright::Error> {
    // JavaScript executed in page to find links whose path ends with /overview (book overview pages)
    let js = r#"() => {
        const anchors = Array.from(document.querySelectorAll('a[href]'));
        const out = [];
        for (const a of anchors) {
            const href = a.getAttribute('href') || '';
            try {
                const url = new URL(href, location.origin);
                if(!url.pathname.endsWith('/overview')) continue;
                const slug = url.pathname.replace(/^\//,'').replace(/\/overview$/,'');
                if(!slug || slug.includes('/')) continue; // ensure single segment slug
                const title = (a.textContent || '').trim();
                if(!title) continue;
                // avoid duplicates (keep first)
                if(!out.find(e => e.slug === slug)) out.push({ slug, title });
            } catch(e) { /* ignore malformed */ }
        }
        return out;
    }"#;
    let books: Vec<BookLink> = page.eval(js).await?;
    Ok(books)
}

/// Perform the entire login flow: load login page, wait for reCAPTCHA, submit credentials, verify dashboard.
pub async fn login() -> Result<(), playwright::Error> {
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?; // Install browsers
    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;
    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;
    page.goto_builder("https://leanpub.com/login")
        .goto()
        .await?;

    // Wait for JS to populate the g-recaptcha hidden field (polling up to ~15s)
    for attempt in 0..30 {
        // 30 * 500ms = 15s max
        let captcha_val: String = page
            .eval(r#"() => {
                const el = document.querySelector("input[name^='g-recaptcha-response'], textarea[name='g-recaptcha-response'], input[name^='g-recaptcha-response-data']");
                return el && el.value ? el.value : '';
            }"#)
            .await
            .unwrap_or_default();
        if !captcha_val.is_empty() {
            println!(
                "reCAPTCHA field populated after {} attempt(s) (~{} ms)",
                attempt + 1,
                (attempt + 1) * 500
            );
            break;
        }
        if attempt == 29 {
            println!("reCAPTCHA field not populated within timeout; proceeding anyway.");
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Evaluate in the page context to extract all input fields within the first form
    let js = r#"() => {
        const form = document.querySelector('form');
        if(!form) { return []; }
        const inputs = Array.from(form.querySelectorAll('input'));
        return inputs.map(i => ({
            name: i.getAttribute('name') || '',
            value: i.value || '',
            type: i.getAttribute('type') || ''
        }));
    }"#;

    let fields: Vec<FormField> = page.eval(js).await?;
    println!("Found {} input fields in login form:", fields.len());
    for f in &fields {
        println!(
            "  name='{}' type='{}' value='{}'",
            f.name, f.field_type, f.value
        );
    }

    // Optionally locate the form action attribute
    let form_action: Option<String> = page.eval("() => { const f = document.querySelector('form'); return f ? f.getAttribute('action') : null; }").await?;
    if let Some(action) = form_action {
        println!("Form action: {}", action);
    }

    // Load credentials from environment
    dotenvy::dotenv().ok();
    let email = std::env::var("LEANPUB_EMAIL").unwrap_or_default();
    let password = std::env::var("LEANPUB_PASSWORD").unwrap_or_default();
    if email.is_empty() || password.is_empty() {
        eprintln!(
            "LEANPUB_EMAIL or LEANPUB_PASSWORD missing in environment; skipping form submission."
        );
        return Ok(());
    }

    // Escape single quotes for JS embedding
    let safe_email = email.replace('\'', "\\'");
    let safe_password = password.replace('\'', "\\'");
    let fill_and_submit = format!(
        r#"() => {{
        const emailInput = document.querySelector("input[name='session[email]']");
        if(emailInput) emailInput.value = '{email}';
        const pwInput = document.querySelector("input[name='session[password]']");
        if(pwInput) pwInput.value = '{password}';
        const form = emailInput ? emailInput.form : document.querySelector('form');
        if(form) {{
            const btn = form.querySelector("input[type=submit],button[type=submit]");
            if(btn) btn.click(); else form.submit();
        }}
        return !!(emailInput && pwInput);
    }}"#,
        email = safe_email,
        password = safe_password
    );

    let filled: bool = page.eval(&fill_and_submit).await?;
    if filled {
        println!("Filled credentials and submitted form.");
    } else {
        eprintln!("Failed to locate form fields to fill.");
    }

    // Poll for navigation / dashboard appearance
    for _ in 0..20 {
        // up to ~10s
        let url: String = page.eval("() => location.href").await.unwrap_or_default();
        if url.contains("author_dashboard") || url.contains("/u/") {
            // heuristic
            println!("Login likely successful. Current URL: {}", url);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Print a snippet of page title and any user menu indicator
    let title: String = page.eval("() => document.title").await.unwrap_or_default();
    println!("Page title after submit: {}", title);
    let user_indicator: Option<String> = page.eval(r#"() => { const el = document.querySelector('[data-test=\"user-menu\"]') || document.querySelector('.user-menu'); return el ? el.textContent : null; }"#).await.unwrap_or(None);
    if let Some(ind) = user_indicator {
        println!("User indicator snippet: {}", ind.trim());
    }

    if !email.is_empty() {
        match verify_login(&page).await? {
            true => match fetch_published_books(&page).await {
                Ok(list) => {
                    println!("Published books ({}):", list.len());
                    for b in list {
                        println!("  {} => {}", b.slug, b.title);
                    }
                }
                Err(e) => eprintln!("Failed to fetch published books: {}", e),
            },
            false => {
                eprintln!("Login failed; exiting.");
                return Ok(());
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), playwright::Error> {
    login().await
}
