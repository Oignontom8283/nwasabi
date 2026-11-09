//! La description des registres donnée au débogueur (profil M d'ARM, numérotation simple 0 à 16).

/// Le fichier XML de la cible, lu par GDB avec `qXfer:features:read`.
pub(super) const TARGET_XML: &str = r#"<?xml version="1.0"?>
<!DOCTYPE target SYSTEM "gdb-target.dtd">
<target version="1.0">
  <architecture>arm</architecture>
  <feature name="org.gnu.gdb.arm.m-profile">
    <reg name="r0" bitsize="32" regnum="0"/>
    <reg name="r1" bitsize="32" regnum="1"/>
    <reg name="r2" bitsize="32" regnum="2"/>
    <reg name="r3" bitsize="32" regnum="3"/>
    <reg name="r4" bitsize="32" regnum="4"/>
    <reg name="r5" bitsize="32" regnum="5"/>
    <reg name="r6" bitsize="32" regnum="6"/>
    <reg name="r7" bitsize="32" regnum="7"/>
    <reg name="r8" bitsize="32" regnum="8"/>
    <reg name="r9" bitsize="32" regnum="9"/>
    <reg name="r10" bitsize="32" regnum="10"/>
    <reg name="r11" bitsize="32" regnum="11"/>
    <reg name="r12" bitsize="32" regnum="12"/>
    <reg name="sp" bitsize="32" type="data_ptr" regnum="13"/>
    <reg name="lr" bitsize="32" regnum="14"/>
    <reg name="pc" bitsize="32" type="code_ptr" regnum="15"/>
    <reg name="xpsr" bitsize="32" regnum="16"/>
  </feature>
</target>
"#;
