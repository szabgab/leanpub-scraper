use playwright::Playwright;

#[tokio::main]
async fn main() -> Result<(), playwright::Error> {
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?; // Install browsers
    let chromium = playwright.chromium();
    let browser = chromium.launcher().headless(true).launch().await?;
    let context = browser.context_builder().build().await?;
    let page = context.new_page().await?;
    page.goto_builder("https://leanpub.com/login").goto().await?;
    

    // Exec in browser and Deserialize with serde
    // let s: String = page.eval("() => location.href").await?;
    // assert_eq!(s, "https://example.com/");
    // page.click_builder("a").click().await?;
    Ok(())
}

