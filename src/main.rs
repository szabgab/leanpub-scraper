use playwright::Playwright;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FormField {
    name: String,
    value: String,
    #[serde(rename = "type")] // 'type' is a keyword in JS/attribute, map it
    field_type: String,
}

#[tokio::main]
async fn main() -> Result<(), playwright::Error> {
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?; // Install browsers
    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;
    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;
    page.goto_builder("https://leanpub.com/login").goto().await?;

    // Wait for JS to populate the g-recaptcha hidden field (polling up to ~15s)
    for attempt in 0..30 { // 30 * 500ms = 15s max
        let captcha_val: String = page
            .eval(r#"() => {
                const el = document.querySelector("input[name^='g-recaptcha-response'], textarea[name='g-recaptcha-response'], input[name^='g-recaptcha-response-data']");
                return el && el.value ? el.value : '';
            }"#)
            .await
            .unwrap_or_default();
        if !captcha_val.is_empty() {
            println!("reCAPTCHA field populated after {} attempt(s) (~{} ms)", attempt + 1, (attempt + 1) * 500);
            break;
        }
        if attempt == 29 { println!("reCAPTCHA field not populated within timeout; proceeding anyway."); }
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
        println!("  name='{}' type='{}' value='{}'", f.name, f.field_type, f.value);
    }

    // Optionally locate the form action attribute
    let form_action: Option<String> = page.eval("() => { const f = document.querySelector('form'); return f ? f.getAttribute('action') : null; }").await?;
    if let Some(action) = form_action { println!("Form action: {}", action); }

    // Load credentials from environment
    dotenvy::dotenv().ok();
    let email = std::env::var("LEANPUB_EMAIL").unwrap_or_default();
    let password = std::env::var("LEANPUB_PASSWORD").unwrap_or_default();
    if email.is_empty() || password.is_empty() {
        eprintln!("LEANPUB_EMAIL or LEANPUB_PASSWORD missing in environment; skipping form submission.");
        return Ok(());
    }

    // Escape single quotes for JS embedding
    let safe_email = email.replace('\'', "\\'");
    let safe_password = password.replace('\'', "\\'");
    let fill_and_submit = format!(r#"() => {{
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
    }}"#, email=safe_email, password=safe_password);

    let filled: bool = page.eval(&fill_and_submit).await?;
    if filled { println!("Filled credentials and submitted form."); } else { eprintln!("Failed to locate form fields to fill."); }

    // Poll for navigation / dashboard appearance
    for _ in 0..20 { // up to ~10s
        let url: String = page.eval("() => location.href").await.unwrap_or_default();
        if url.contains("author_dashboard") || url.contains("/u/") { // heuristic
            println!("Login likely successful. Current URL: {}", url);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Print a snippet of page title and any user menu indicator
    let title: String = page.eval("() => document.title").await.unwrap_or_default();
    println!("Page title after submit: {}", title);
    let user_indicator: Option<String> = page.eval(r#"() => { const el = document.querySelector('[data-test=\"user-menu\"]') || document.querySelector('.user-menu'); return el ? el.textContent : null; }"#).await.unwrap_or(None);
    if let Some(ind) = user_indicator { println!("User indicator snippet: {}", ind.trim()); }



    Ok(())
}

