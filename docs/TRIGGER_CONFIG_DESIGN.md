# Trigger config design: what the schema can express

Companion to [README.md](./README.md#field-reference). The README lists fields; this document
covers what the shape of the trigger schema lets a machine config express, and what it adds over
the legacy flat-trigger layout (`Triggers/{label}` with `Start_Value` / `Stop_Value`). How a
consumer evaluates rules at runtime (matching, edge detection, cooldowns, dispatch) belongs to
that consumer; capture's behaviour is summarised in [CONSUMERS.md](./CONSUMERS.md#capture).

Examples come from the datastream test profiles (`aconity-datastream/config/h5_schema.json`),
which are written as real 1.2 files and loaded by capture.

## Layout at a glance

The trigger tree has three levels. Each level maps to one model struct.

**Level 1: `Trigger/` group -> `TriggerConfig`** (global switches)

```text
Trigger/                                   TriggerConfig
  @Triggers_Enabled            u32 flag      enabled              bool
  @Trigger_Stop_Ceiling_Layers u32           stop_ceiling_layers  Option<u32>   (0 = none)
  {Rule_Label}/  ...                         rules                Vec<Rule>     (level 2)
```

**Level 2: `{Rule_Label}/` group -> `Rule`** (one per rule: what to do, and when it may fire)

```text
{Rule_Label}/                              Rule
  identity
    @Rule_Id                   u32 >= 1      id                   u32
    @Rule_Label                utf8          label                String        (= group name)
    @Description               utf8          description          String
  behaviour
    @Action                    utf8          action               RuleAction    [CaptureStart / CaptureStop]
    @Rule_Enabled              u32 flag      enabled              bool
    @Priority                  u32           priority             u32           (lower wins, 0 highest)
    @Combinator                utf8          combinator           Option<Combinator> [And / Or]
  throttling
    @Cooldown_Period           u32           cooldown_ms          u32
    @Max_Fires_Per_Job         u32           max_fires_per_job    Option<u32>   (0 = unlimited)
  {Condition_Label}/  ...                    conditions           Vec<Condition> (level 3)
```

**Level 3: `{Condition_Label}/` group -> `Condition`** (one per condition: which signal, and how
to compare it)

```text
{Condition_Label}/                         Condition
  identity
    @Condition_Id              u32 >= 1      id                   u32
    @Condition_Label           utf8          label                String        (= group name)
    @Condition_Enabled         u32 flag      enabled              bool
  signal source
    @Signal                    utf8          signal               String
    @Event                     utf8          event                String        (empty = any)
    @Subsystem                 utf8          subsystem            String        (empty = any)
    @Component                 utf8          component            String        (empty = any)
  comparison                                 evaluation           Evaluation (tagged union)
    @Evaluation_Mode           utf8            selects the variant: [String / Numeric]
    @Value                     utf8            String.value       | Numeric.threshold (finite number)
    @Case_Sensitivity          utf8            String.case_sensitivity   (blank in Numeric mode)
    @Comparison_Operator       utf8            Numeric.operator          (blank in String mode)
    @Epsilon                   f64             Numeric.epsilon           (0.0 in String mode)
```

A rule owns one or more conditions; a condition belongs to exactly one rule. The complete file
tree, with the Client and Pipe groups and example values, is in
[Full reference tree](#full-reference-tree).

## The legacy shape, for contrast

The legacy layout stored one flat trigger per group: a single `Signal` (plus optional `Event`,
`Subsystem`, `Component`), a `Start_Value` and a `Stop_Value`, `Case_Sensitivity`,
`Cooldown_Period`, `Max_Fires_Per_Job`, `Rule_Enabled`, and optional `ID` / `Trigger_Label`
strings. Nothing was required, and the group name was the only real identity. The codec still
reads it, converting each trigger into a Start rule and a Stop rule (see README, Legacy layout).

## 1. Several conditions per rule, joined by a combinator

A legacy trigger watches exactly one signal. A 1.2 rule holds a list of conditions joined by
`Combinator` (`And` / `Or`). Profile `tm`'s `Job Stop-Finish` is one `CaptureStop` rule with
`Combinator Or` over `RunState == "finished"` and `RunState == "stopped"`: Aconity's automatic
completion and a manual stop from AconityStudio, one logical stop condition. The legacy shape
needs two unrelated triggers for this.

`Combinator` is required when a rule has more than one condition. If it is blank in that case the
reader reports an Advisory warning, because consumers then evaluate only one of the conditions.

## 2. Conditions from different sources in one rule

Each condition names its own signal source, so one rule can mix machine state and sensor data.
Profile `tm`'s `NumericalTesting` (`CaptureStop`, `Or`) combines a numeric
`sensor_temperature >= 200.0` from the oxygen sensor with two `RunState` string checks. A legacy
trigger is pinned to one signal.

## 3. One action per rule

Legacy start and stop were two values on one trigger. In 1.2 every rule has exactly one `Action`,
so starting and stopping are separate rules, each with its own id, enable flag, priority, cooldown
and fire limit, even when they watch the same signal (`Job Start` and `Job Stop-Finish` both watch
`RunState`).

## 4. String or numeric comparison, chosen per condition

The legacy layout only had `Case_Sensitivity`: string matching, with no way to say
`oxygen_level < 5.0`. Each 1.2 condition chooses its mode:

- **String**: `Value` is compared as text using `Case_Sensitivity`
  `[Exact / Contains / ExactCaseSensitive / ContainsCaseSensitive]`.
- **Numeric**: `Value` must be a finite number; it is compared with `Comparison_Operator`
  `[Equal / NotEqual / GreaterThan / LessThan / GreaterThanOrEqual / LessThanOrEqual]` within
  `Epsilon` (`0` = the consumer's default tolerance).

The model enforces the split: `evaluation` is a tagged union, so the fields the mode does not use
cannot leak into evaluation. On disk every condition still carries all three comparison fields;
the unused ones are blank (or `0.0` for `Epsilon`). A Numeric `Value` that is not a number is
rejected at load time rather than failing silently at runtime.

## 5. Throttling and conflict resolution

`Cooldown_Period` (ms) and `Max_Fires_Per_Job` (`0` = unlimited) limit how often each rule fires.
`Priority` resolves conflicts: when a Start rule and a Stop rule are both eligible at the same
moment, the rule with the **lower** priority value wins, with `0` the highest precedence. Profile
`demo` gives `Job Start` and `Job Stop` the same priority to exercise the tie-break (capture
favours Start on a tie). Priority is required, so no rule silently takes the highest precedence
by omission.

## 6. Two-level enablement

`Rule_Enabled` and `Condition_Enabled` are required flags, and `Triggers_Enabled` on the
`Trigger` group is a separate global switch. One misbehaving rule or condition can be disabled
without touching the others, and the whole trigger system can be turned off without editing every
rule. Profile `demo` covers both a disabled rule and a disabled condition inside an enabled rule.

## 7. Stable numeric identity, separate from the label

`Rule_Id` and `Condition_Id` are required, `>= 1` and unique (rule ids across the file, condition
ids within their rule). Labels are also required and unique, because they are the group names.
Consumers key state (cooldowns, fire counts, overrides) on ids, so renaming a rule does not reset
or misattribute its state. Profile `full_range` uses non-sequential ids (rules 3 and 7, conditions
4 and 9) to check that nothing depends on position.

## 8. Transport, session and rules are separate groups

`Pipe` (IPC transport and its security), `Client` (OPC UA session) and `Trigger` (rules) stay
three independent groups, as they were in the legacy layout. A deployment can change pipe
security without touching session tuning or rules.

## Full reference tree

Every attribute of a schema 1.2 file, from the file root down, with example values. Groups are
separated by a blank line; `@` marks an attribute, `dtype` follows the name. Field rules are in
[README.md](./README.md#field-reference).

```text
/                                              machine config file, File_Version 1.1
├── Machine/ ...                               modelled by MCL core (not shown)
└── Extensions/
    ├── ClearBox/ ...                          another extension (not shown)
    │
    └── TM_OPCUA/                              this extension
        @Opcua_Schema_Version          utf8    "1.2"
        │
        ├── Client/                            OPC UA session and subscription
        │     connection and security
        │     @Server_URL              utf8    "opc.tcp://10.0.0.5:62541/TM_OPCUA_Template/TelemetryServer"
        │     @Auth_Mode               utf8    "UsernamePassword"
        │     @Security_Mode           utf8    "SignAndEncrypt"
        │     @Security_Policy         utf8    "ECC_nistP384"        ("" = Auto)
        │     discovery
        │     @Machine_Profile         utf8    "Aconity"             ("" = none)
        │     @Root_Node               utf8    "MachineFleet"        ("" = auto-discover)
        │     @BFS_Max_Depth           u32     16
        │     subscription
        │     @Sampling_Interval       i32     50                    (-1 inherit, 0 fastest)
        │     @Publish_Interval        u32     250
        │     @Keep_Alive_Count        u32     240
        │     @Lifetime_Count          u32     2400
        │     @Queue_Policy            utf8    "DiscardOldest"
        │     @Queue_Size_Data_Change  u32     50
        │     @Queue_Size_Events       u32     10
        │     session timing
        │     @Session_Timeout         u32     60000
        │     @Reconnect_Interval      u32     1000
        │     @Sync_Loop_Interval_Initial u32  1000
        │     @Sync_Loop_Interval_Settled u32  30000
        │
        ├── Pipe/                              capture <-> client-proj transport
        │     @Pipe_Enabled            u32     1
        │     @Pipe_Name               utf8    "\\.\pipe\opc_ua_client_pipe"
        │     @Configure_Client        u32     1
        │     security
        │     @User_Access_Level       utf8    "CurrentUserOnly"
        │     @Min_Integrity_Level     utf8    "Medium"
        │     limits
        │     @Buffer_Size             u32     65536
        │     @Inbound_Rate_Limit      i32     100                   (0 / -1 = no cap)
        │     @Max_Inbound_Message_Size u32    4096                  (0 = no limit)
        │
        └── Trigger/                           TriggerConfig
              @Triggers_Enabled        u32     1
              @Trigger_Stop_Ceiling_Layers u32 3                     (0 = none)
              │
              ├── Job Start/                   Rule
              │     @Rule_Id           u32     1
              │     @Rule_Label        utf8    "Job Start"
              │     @Description       utf8    "Start when the build is running"
              │     @Action            utf8    "CaptureStart"
              │     @Rule_Enabled      u32     1
              │     @Priority          u32     1
              │     @Combinator        utf8    ""                    (single condition)
              │     @Cooldown_Period   u32     500
              │     @Max_Fires_Per_Job u32     0                     (0 = unlimited)
              │     │
              │     └── Running/                   Condition (String)
              │           @Condition_Id        u32   1
              │           @Condition_Label     utf8  "Running"
              │           @Condition_Enabled   u32   1
              │           @Signal              utf8  "RunState"
              │           @Event               utf8  ""
              │           @Subsystem           utf8  ""
              │           @Component           utf8  ""
              │           @Evaluation_Mode     utf8  "String"
              │           @Value               utf8  "running"
              │           @Case_Sensitivity    utf8  "Exact"
              │           @Comparison_Operator utf8  ""              (unused in String mode)
              │           @Epsilon             f64   0.0             (unused in String mode)
              │
              └── Job Stop/                    Rule
                    @Rule_Id           u32     2
                    @Rule_Label        utf8    "Job Stop"
                    @Description       utf8    ""
                    @Action            utf8    "CaptureStop"
                    @Rule_Enabled      u32     1
                    @Priority          u32     0                     (wins a tie-break over Start)
                    @Combinator        utf8    "Or"
                    @Cooldown_Period   u32     0
                    @Max_Fires_Per_Job u32     0
                    │
                    ├── Finished/                  Condition (String)
                    │     @Condition_Id        u32   1
                    │     @Condition_Label     utf8  "Finished"
                    │     @Condition_Enabled   u32   1
                    │     @Signal              utf8  "RunState"
                    │     @Event               utf8  ""
                    │     @Subsystem           utf8  ""
                    │     @Component           utf8  ""
                    │     @Evaluation_Mode     utf8  "String"
                    │     @Value               utf8  "finished"
                    │     @Case_Sensitivity    utf8  "Exact"
                    │     @Comparison_Operator utf8  ""
                    │     @Epsilon             f64   0.0
                    │
                    └── Over Temperature/          Condition (Numeric)
                          @Condition_Id        u32   2
                          @Condition_Label     utf8  "Over Temperature"
                          @Condition_Enabled   u32   1
                          @Signal              utf8  "sensor_temperature"
                          @Event               utf8  "SensorEvents"
                          @Subsystem           utf8  "Gas"
                          @Component           utf8  "process_chamber::gas_management::oxygen_sensor::1"
                          @Evaluation_Mode     utf8  "Numeric"
                          @Value               utf8  "200.0"
                          @Case_Sensitivity    utf8  ""              (unused in Numeric mode)
                          @Comparison_Operator utf8  "GreaterThanOrEqual"
                          @Epsilon             f64   0.0             (0 = consumer default)
```

Attributes inside a group are unordered on disk; they are grouped here by purpose for reading.
