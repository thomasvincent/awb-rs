-- Fix Double Spaces Plugin
-- Replaces multiple consecutive spaces with a single space

description = "Replaces multiple consecutive spaces with a single space"

function transform(text)
    -- Replace 2 or more spaces with a single space
    local result = text:gsub("  +", " ")

    -- Also fix spaces before punctuation
    result = result:gsub(" +([%.,%?!;:])", "%1")

    return result
end
