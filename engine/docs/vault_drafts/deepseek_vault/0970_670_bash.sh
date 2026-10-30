rule = {
matches = {
{
{ "node.nick", "equals", "ASUS VK222H" },
},
},
apply_properties = {
["node.description"] = "Монитор ASUS",
["priority.driver"] = 1000,
["priority.session"] = 1000,
        ["node.pause-on-idle"] = false,  -- Не приостанавливать при простое
},
}

5.3. Перезапустим WirePlumber:

bash
