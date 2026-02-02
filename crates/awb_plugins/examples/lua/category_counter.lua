-- Category Counter Plugin
-- Adds an HTML comment showing the number of categories

description = "Adds a comment showing the total number of categories"

function transform(text)
    -- Get all categories using the mw helper
    local categories = mw.categories(text)
    local count = #categories

    -- Only add comment if there are categories
    if count > 0 then
        -- Add comment at the end
        return text .. "\n<!-- " .. count .. " categor" .. (count == 1 and "y" or "ies") .. " -->"
    end

    return text
end
