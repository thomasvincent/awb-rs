-- Remove External Links Plugin
-- Removes all external links from wikitext

description = "Removes all external links [http://...] from wikitext"

function transform(text)
    -- Remove external links in format [http://example.com text]
    text = text:gsub("%[https?://[^%]]+%]", "")

    -- Remove bare URLs
    text = text:gsub("https?://[%w%.%-_/%%?&=]+", "")

    return text
end
