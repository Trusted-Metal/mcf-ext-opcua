# Trigger config design: what the schema can express

Companion to [README.md](./README.md#field-reference). The README lists fields; this document
covers what the shape of the trigger schema lets a machine config express, and what it adds over
the legacy flat-trigger layout (`Triggers/{label}` with `Start_Value` / `Stop_Value`). How a
consumer evaluates rules at runtime (matching, edge detection, cooldowns, dispatch) belongs to
that consumer; capture's behaviour is summarised in [CONSUMERS.md](./CONSUMERS.md#capture).

Examples come from the datastream test profiles (`aconity-datastream/config/h5_schema.json`),
which are written as real 1.2 files and loaded by capture.

## Layout at a glance

```text
Trigger/                                   TriggerConfig
  @Triggers_Enabled            u32 flag      enabled              bool
  @Trigger_Stop_Ceiling_Layers u32           stop_ceiling_layers  Option<u32>
  {Rule_Label}/                              rules: Vec<Rule>
    @Rule_Id                   u32 >= 1        id                 u32
    @Rule_Label                utf8            label              String
    @Action                    utf8            action             RuleAction [CaptureStart / CaptureStop]
    @Rule_Enabled              u32 flag        enabled            bool
    @Priority                  u32             priority           u32 (lower wins, 0 highest)
    @Combinator                utf8            combinator         Option<Combinator> [And / Or]
    @Cooldown_Period           u32             cooldown_ms        u32
    @Max_Fires_Per_Job         u32             max_fires_per_job  Option<u32>
    @Description               utf8            description        String
    {Condition_Label}/                         conditions: Vec<Condition>
      @Condition_Id            u32 >= 1          id               u32
      @Condition_Label         utf8              label            String
      @Condition_Enabled       u32 flag          enabled          bool
      @Signal / @Event / @Subsystem / @Component   signal source
      @Evaluation_Mode         utf8              evaluation:  String  { value, case_sensitivity }
      @Value                   utf8                         | Numeric { threshold, operator, epsilon }
      @Case_Sensitivity        utf8              (String mode)
      @Comparison_Operator     utf8              (Numeric mode)
      @Epsilon                 f64               (Numeric mode)
```

A rule owns one or more conditions; a condition belongs to exactly one rule.

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
reader warns, because consumers then evaluate only one of the conditions.

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
