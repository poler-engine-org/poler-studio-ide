rules = {
{
matches = {
{
{ "device.name", "equals", "alsa_card.pci-0000_01_00.1" },
},
},
apply_properties = {
["device.profile"] = "hdmi-stereo",
["device.description"] = "HDMI",
},
},
{
matches = {
{
{ "node.name", "equals", "alsa_output.pci-0000_01_00.1.hdmi-stereo" },
},
},
apply_properties = {
["node.nick"] = "HDMI",
["node.description"] = "HDMI",
      ["priority.session"] = 1000,  -- Высокий приоритет
},
},
}
