alsa_monitor.rules = {
{
matches = {
{
{ "device.name", "equals", "alsa_card.pci-0000_01_00.1" },
},
},
apply_properties = {
["api.alsa.use-acp"] = true,
["api.alsa.acp.auto-profile"] = false,
["api.alsa.acp.auto-port"] = false,
["device.profile-set"] = "hdmi-stereo",
["device.profile"] = "hdmi-stereo",
},
},
}

node_monitor.rules = {
{
matches = {
{
{ "node.name", "equals", "alsa_output.pci-0000_01_00.1.hdmi-stereo" },
},
},
apply_properties = {
["node.nick"] = "HDMI-ASUS",
["priority.driver"] = 1000,
["priority.session"] = 1000,
      ["session.suspend-timeout-seconds"] = 0,  # Не отключать никогда
},
},
}

Перезапустите WirePlumber:

bash
