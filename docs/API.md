# SKS3200-8E2X Web API Reference

Complete API surface discovered by reverse-engineering the switch web UI.
All endpoints return JSON. Authentication uses HTTP cookies via `GET /authorize`.

## Authentication

**`GET /authorize`**

Params: `loginusr` (MD5 of username), `loginpwd` (MD5 of password)

```
GET /authorize?loginusr=21232f297a57a5a743894a0e4a801fc3&loginpwd=6372aa58d8a53b13c77cd51b27caa0be
```

Success: Returns redirect to `index.html?page=` and sets `session` + `user` cookies.
Failure: Returns redirect to `login.html`.

**Session cookie**: `session=<hex>` (http-only, SameSite=Lax)
**User cookie**: `user=<md5(username)>` (SameSite=Lax)

---

## Read-only Endpoints

### 1. System Information

**`GET /status.json`**

```json
{
  "temperature": "53",
  "sys_ipv4": "192.168.100.7",
  "sys_macaddr": "8C:A6:82:71:8F:B4",
  "fw_ver": "2.0.0.3",
  "hw_ver": "A0",
  "des": "SKS3200-8E2X"
}
```

### 2. Network Settings

**`GET /network_settings.json`**

```json
{
  "ipAddress": "192.168.100.7",
  "netmask": "255.255.255.0",
  "gateway": "192.168.100.254",
  "dhcpEnabled": "0",
  "dnsServer": "8.8.8.8",
  "autoDnsEnabled": "1"
}
```

### 3. Port Settings

**`GET /port_setting_load.json`**

```json
{
  "PortNum": "10",
  "PortMode": "PORT_MODE_8_PLUS_2",
  "Port_1": {
    "EEE_Status": "eee_inactive",
    "Port_Id": "1",
    "Port_Status": "Enabled",
    "Spd_Duplex_Cfg": "Auto",
    "Spd_Duplex_Actual": "Link Down",
    "Flow_Ctrl_Cfg": "On",
    "Flow_Ctrl_Actual": "On"
  }
}
```

Ports 1-8 are RJ45 2.5G (may have `EEE_Status: eee_active`).
Ports 9-10 are SFP+ 10G (extra fields: `Combo`, `Is_extPhy`).

`EEE_Status` values: `eee_active`, `eee_inactive`, `eee_na`.
`Spd_Duplex_Actual` values: `Link Down`, `100MbpsFull`, `1000MbpsFull`, `2500MbpsFull`.

### 4. Port Statistics

**`GET /port_statistics.json`**

```json
{
  "PortNum": "10",
  "Port_1": {
    "Port_Id": "1",
    "Port_Status": "Enabled",
    "Link_Status": "Link Down",
    "TxGoodPkt": "0",
    "TxBadPkt": "0",
    "RxGoodPkt": "0",
    "RxBadPkt": "0"
  }
}
```

`Link_Status` values: `Link Down`, `100MbpsFull`, `1000MbpsFull`, `2500MbpsFull`.

### 5. Dynamic MAC Table

**`GET /mac_get_dynamic_mac_entries.json`**

Response uses non-standard `data: [...]` line format:

```
data: [{"Dynamic_idx":1,"Dynamic_mac_addr":"00:0E:58:85:04:82","Dynamic_vlan_id":1,"Dynamic_fid":0,"Dynamic_portid":8,"Dynamic_age_timer":244}]
data: [{"Dynamic_idx":2,"Dynamic_mac_addr":"DC:A6:32:43:C4:B0","Dynamic_vlan_id":1,"Dynamic_fid":0,"Dynamic_portid":6,"Dynamic_age_timer":244}]
```

Multiple `data:` lines may be present.

### 6. Static MAC Table

**`GET /mac_get_static_mac_entries.json`**

Same `data: [...]` format:

```json
data: [{"Static_idx":1,"Static_mac_addr":"00:11:22:33:44:55","Static_vlan_id":1,"Static_portid":1}]
```

### 7. Link Aggregation (Trunk)

**`GET /port_trunk_cfg.json`**

```json
{
  "PortNum": 10,
  "system_priority": 32768,
  "Port_1": {
    "portTypeId_1": 0,
    "portPriorityId_1": 128,
    "lacpTimeoutId_1": 0,
    "Port_1_grpInd": 0,
    "Port_1_state": 0
  }
}
```

`portTypeId`: 0 = Static, 1 = LACP
`Port_X_state`: 0 = not in group, 1 = in group
`Port_X_grpInd`: aggregation group index

### 8. Loop Protection Status

**`GET /port_loop_status.json`**

```json
{
  "PortNum": "10",
  "Violdetd_1": "0",
  ...
  "Violdetd_10": "0"
}
```

`0` = no violation, otherwise violation detected.

### 9. Loop Protection Config

**`GET /port_lock_cfg.json`**

```json
{
  "PortNum": "10",
  "Port_1": {"Violdetd_1": "0"},
  "cPrev": "on",
  "detect_enable": "1",
  "time_interval": "10",
  "recover_time": "2"
}
```

### 10. Spanning Tree Protocol

**`GET /stp.json`**

```json
{
  "stp_enable": "0",
  "stp_rstp_mode": "RSTP",
  "num_ports": "10",
  "Port_1": {
    "Stp_Edge_1": "0",
    "Stp_Status_1": "Forward",
    "Hw_Port_Id_1": "Port 1"
  }
}
```

`stp_enable`: `0` = disabled, `1` = enabled.

### 11. Port VLAN

**`GET /port_vlan.json`**

```json
{
  "PortNum": 10,
  "Port_1": {
    "Port_Id": 1,
    "PVID": 1,
    "Frame_Type": 0
  }
}
```

`Frame_Type`: 0 = All, 1 = Tagged, 2 = Untagged

### 12. All Port PVIDs (compact)

**`GET /all_port_pvid.json`**

```json
{
  "port_pvids": [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
}
```

Index 0 is unused (1-indexed ports), indices 1-10 correspond to ports 1-10.

### 13. Storm Control

**`GET /storm_ctrl_cfg.json`**

```json
{
  "portnum": 10,
  "ports": [
    {"port_id": 1, "sctrl_bcast": 0, "sctrl_mcast": 0, "sctrl_unucast": 0, "sctrl_unmcast": 0}
  ]
}
```

Values are rate limits in Kbps (0 = disabled).

### 14. Port Mirror

**`GET /port_mirror.json`**

```json
{
  "PortNum": "10",
  "MonitoringPortId": "0",
  "Port_1": {
    "Port_Id": "1",
    "Ingress_Status": "Disabled",
    "Egress_Status": "Disabled"
  }
}
```

`MonitoringPortId`: `0` = disabled, otherwise the destination port.

### 15. IGMP Snooping

**`GET /igmp_config.json`**

```json
{
  "igmp": "on",
  "fast_leave": "on",
  "report_flood": "off"
}
```

### 16. System Time

**`GET /systemtime_settings.json`**

Response format TBD (not yet captured).

---

## Write Endpoints (Phase 2)

| Endpoint | Method | Body / Params |
|---|---|---|
| `POST /save_all_configs.json` | POST | `{}` (empty) |
| `POST /apply_user_port_setting.json` | POST | Port config |
| `POST /set_des.json` | POST | `{des: "..."}` |
| `POST /system_reboot.json` | POST | `{}` |
| `POST /factory_reset.json` | POST | `{}` |
| `POST /network_settings_ipv4.json` | POST | IPv4 config |
| `POST /clear_statistics.json` | GET | (clears port counters) |
| `POST /mac_clear_dynamic_mac_entries.json` | POST | (clears MAC table) |

---

## Notes

- Port numbering: 1-8 = RJ45 2.5G, 9-10 = SFP+ 10G
- All ports are in VLAN 1 by default
- No CSRF protection observed
- Config download: `GET /config/download`
- Config upload: `POST /config/upload`
- Firmware download: `GET /firmware/download`
- Firmware upgrade: `POST /firmware/upgrade`
