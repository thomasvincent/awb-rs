-- Stub Detector Plugin
-- Detects if an article is a stub and adds a stub template if missing

description = "Adds {{stub}} template to short articles if not present"

-- Minimum word count to not be considered a stub
local MIN_WORDS = 50

function count_words(text)
    -- Remove wiki markup for more accurate count
    local cleaned = text:gsub("%[%[[^%]]+%]%]", "") -- Remove wikilinks
    cleaned = cleaned:gsub("{{[^}]+}}", "") -- Remove templates
    cleaned = cleaned:gsub("<[^>]+>", "") -- Remove HTML tags

    local count = 0
    for _ in cleaned:gmatch("%S+") do
        count = count + 1
    end
    return count
end

function has_stub_template(text)
    return text:match("{{[Ss]tub") ~= nil
end

function transform(text)
    -- Don't process redirects
    if mw.is_redirect(text) then
        return text
    end

    -- Count words
    local word_count = count_words(text)

    -- If article is short and doesn't have stub template, add it
    if word_count < MIN_WORDS and not has_stub_template(text) then
        return text .. "\n\n{{stub}}"
    end

    return text
end
