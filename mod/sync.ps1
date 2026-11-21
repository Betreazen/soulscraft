# Fills the mod package's resources (mod\soulscraft\assets and mod\soulscraft\data) and the rules
# crate's data from the one source of truth:
# data\mobs.json and assets\ (rigs, textures, item models from assets\models\build.py). Never edit
# the copies; change the source and run this again. The output is deterministic: running it twice
# changes nothing, so `git status` shows only what the sources changed.
#   .\mod\sync.ps1          write the copies
#   .\mod\sync.ps1 -Check   fail if a copy differs from its source (before a commit or a stage)
param([switch]$Check)
$ErrorActionPreference = 'Stop'

$Repo = Split-Path $PSScriptRoot
$Content = Join-Path $PSScriptRoot 'soulscraft'
$Utf8 = [Text.UTF8Encoding]::new($false)
$script:Changed = @()

function Write-Copy([string]$Path, [byte[]]$Bytes) {
    $old = if (Test-Path $Path) { [IO.File]::ReadAllBytes($Path) } else { $null }
    if ($null -ne $old -and [Collections.StructuralComparisons]::StructuralEqualityComparer.Equals($old, $Bytes)) { return }
    $script:Changed += $Path.Substring($Repo.Length + 1)
    if ($Check) { return }
    New-Item -ItemType Directory -Force (Split-Path $Path) | Out-Null
    [IO.File]::WriteAllBytes($Path, $Bytes)
}

function Write-Text([string]$Path, [string]$Text) {
    Write-Copy $Path $Utf8.GetBytes($Text.Replace("`r`n", "`n"))
}

function Copy-File([string]$From, [string]$To) {
    Write-Copy $To ([IO.File]::ReadAllBytes($From))
}

function Json([object]$Value) {
    ($Value | ConvertTo-Json -Depth 10).Replace("`r`n", "`n") + "`n"
}

$mobs = Get-Content -Raw -Encoding utf8 (Join-Path $Repo 'data\mobs.json') | ConvertFrom-Json

# The numbers: the crate compiles this copy in (src/data.rs); RustCraft and others read it from the mod
# at horizon-mods\soulscraft\data\soulscraft\mobs.json (integration.md, 4.1).
Copy-File (Join-Path $Repo 'data\mobs.json') (Join-Path $Content 'data\soulscraft\mobs.json')

# Entity types on the zombie template (docs/mods.md: a living type copies zombie, villager or the
# wandering trader) with the base numbers of mobs.json; the server sets P's growth on each mob.
$lang = [ordered]@{}
foreach ($m in $mobs.mobs) {
    $attrs = [ordered]@{ 'minecraft:max_health' = [double]$m.stats.health
        'minecraft:attack_damage' = [double]$m.stats.damage
        'minecraft:armor' = [double]$m.stats.armor }
    if ($null -ne $m.stats.speed) { $attrs['minecraft:movement_speed'] = [double]$m.stats.speed }
    $type = [ordered]@{ template = 'minecraft:zombie'; attributes = $attrs }
    Write-Text (Join-Path $Content "data\soulscraft\horizon\entity_type\$($m.id).json") (Json $type)
    $lang["entity.soulscraft.$($m.id)"] = $m.name
}
# Round 11: exactly the undead types (mobs.json `undead`) in the vanilla undead tags, so RustCraft's
# turrets, thrown potions and the engine's effects agree (horizon-mob-models.md, 12.4).
if (@($mobs.mobs | Where-Object { $_.undead -isnot [bool] }).Count) { throw 'data\mobs.json: every mob needs undead true or false' }
$undead = @($mobs.mobs | Where-Object { $_.undead } | ForEach-Object { $_.key })
foreach ($tag in 'undead', 'inverted_healing_and_harm') {
    Write-Text (Join-Path $Content "data\minecraft\tags\entity_type\$tag.json") `
        (Json ([ordered]@{ replace = $false; values = $undead }))
}

# The weapons the rigs hold (socket items): declared items, so their models resolve. The brute's
# maul is also the reward of tier 3, so it is declared with the tier rewards further down.
$weapons = [ordered]@{ rusty_sword = 'minecraft:iron_sword'; stalker_sickle = 'minecraft:iron_sword' }
$names = @{ rusty_sword = 'Ржавый меч'; stalker_sickle = 'Серп ловчего' }
foreach ($w in $weapons.Keys) {
    Write-Text (Join-Path $Content "data\soulscraft\horizon\item\$w.json") (Json ([ordered]@{ template = $weapons[$w] }))
    Write-Text (Join-Path $Content "assets\soulscraft\items\$w.json") `
        (Json ([ordered]@{ model = [ordered]@{ type = 'minecraft:model'; model = "soulscraft:item/$w" } }))
    $lang["item.soulscraft.$w"] = $names[$w]
}
foreach ($f in Get-ChildItem (Join-Path $Repo 'assets\soulscraft\models\item') -Filter *.json) {
    if ($weapons.Contains($f.BaseName)) { Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\models\item\$($f.Name)") }
}
foreach ($f in Get-ChildItem (Join-Path $Repo 'assets\soulscraft\textures\item') -Filter *.png) {
    if ($weapons.Contains($f.BaseName)) { Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\item\$($f.Name)") }
}

# The shared bleed (data/mobs.json effects.bleeding): a content mod's mob effect; its damage is
# the bundle's (native_apply_effect_tick).
Write-Text (Join-Path $Content "data\soulscraft\horizon\mob_effect\bleeding.json") `
    (Json ([ordered]@{ category = 'harmful'; color = 0x8A0303 }))
$lang['effect.soulscraft.bleeding'] = 'Кровотечение'
# Its HUD icon (textures/mob_effect/<id>.png, 18×18, from the art pipeline).
foreach ($f in Get-ChildItem (Join-Path $Repo 'assets\soulscraft\textures\mob_effect') -Filter *.png) {
    Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\mob_effect\$($f.Name)")
}

# Rigs and the mob textures.
foreach ($f in Get-ChildItem (Join-Path $Repo 'assets\rigs') -Filter *.rig.json) {
    Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\rigs\$($f.Name)")
}
foreach ($f in Get-ChildItem (Join-Path $Repo 'assets\textures\entity') -Filter *.png) {
    Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\entity\$($f.Name)")
}

# ---- The altar (docs/design/06-altar.md, data/altar.json): blocks, banners, the summon item ----
# The crate compiles these in from the package's data/soulscraft (src/altar.rs, src/event.rs,
# src/book.rs, src/loot.rs), as it does mobs.json; nothing reads them as content.
Copy-File (Join-Path $Repo 'data\altar.json') (Join-Path $Content 'data\soulscraft\altar.json')
Copy-File (Join-Path $Repo 'data\trophies.json') (Join-Path $Content 'data\soulscraft\trophies.json')
Copy-File (Join-Path $Repo 'data\book.json') (Join-Path $Content 'data\soulscraft\book.json')
# Chest horns and shards, zombies' Rotten Horn (src/loot.rs, docs/design/11-summons-rewards.md).
Copy-File (Join-Path $Repo 'data\loot.json') (Join-Path $Content 'data\soulscraft\loot.json')
$altar =Get-Content -Raw -Encoding utf8 (Join-Path $Repo 'data\altar.json') | ConvertFrom-Json
$art = Join-Path $Repo 'assets\soulscraft'

# A model's outline in blocks: the box round its elements, inside 0..1.5 (a piece on a slab is
# drawn 8 pixels down into the slab; its box stops at its own block's floor).
function Model-Box([string]$Model) {
    $m = Get-Content -Raw (Join-Path $art "models\block\$Model.json") | ConvertFrom-Json
    $lo = @(16.0, 16.0, 16.0); $hi = @(0.0, 0.0, 0.0)
    foreach ($e in $m.elements) {
        for ($i = 0; $i -lt 3; $i++) {
            $lo[$i] = [Math]::Min($lo[$i], [Math]::Min([double]$e.from[$i], [double]$e.to[$i]))
            $hi[$i] = [Math]::Max($hi[$i], [Math]::Max([double]$e.from[$i], [double]$e.to[$i]))
        }
    }
    if (@($m.elements).Count -eq 0) { return @(0.0, 0.0, 0.0, 1.0, 1.0, 1.0) }
    $c = { param($v) [Math]::Round([Math]::Min(1.5, [Math]::Max(0.0, $v / 16.0)), 4) }
    $box = @((& $c $lo[0]), (& $c $lo[1]), (& $c $lo[2]), (& $c $hi[0]), (& $c $hi[1]), (& $c $hi[2]))
    # A box that is flat after clamping (all of it below the floor) keeps a sliver to click.
    if ($box[4] -le $box[1]) { $box[4] = [Math]::Min(1.5, $box[1] + 0.125) }
    return $box
}

# The box turned as the blockstate turns the model (`y` degrees, clockwise seen from above).
function Turn-Box($b, [int]$Y) {
    $pts = @(@($b[0], $b[2]), @($b[3], $b[5]))
    for ($k = 0; $k -lt (($Y / 90) % 4); $k++) { $pts = $pts | ForEach-Object { , @((1.0 - $_[1]), $_[0]) } }
    $xs = $pts | ForEach-Object { $_[0] }; $zs = $pts | ForEach-Object { $_[1] }
    return @(($xs | Measure-Object -Minimum).Minimum, $b[1], ($zs | Measure-Object -Minimum).Minimum,
        ($xs | Measure-Object -Maximum).Maximum, $b[4], ($zs | Measure-Object -Maximum).Maximum) |
        ForEach-Object { [Math]::Round([double]$_, 4) }
}

# The block file's keys as Horizon 7e4e421 reads them (`deny_unknown_fields`:
# SRC/crates/horizon-mods/src/content.rs, `Block` and `BlockProperties`): an unknown key makes the
# engine drop the whole content mod, so sync fails here first. (docs/mods.md is ahead of 7e4e421
# and shows `occludes` at the top level; on 7e4e421 it is a property.)
$BlockKeys = @('template', 'properties', 'shapes', 'collision_shapes', 'outline_context_shapes', 'collision_context_shapes',
    'interaction_shapes', 'state_properties', 'default_state', 'connects_to', 'thickness', 'wood_type', 'wall_block', 'item', 'components')
$BlockPropertyKeys = @('hardness', 'explosion_resistance', 'light', 'requires_tool', 'friction', 'map_color', 'sounds',
    'random_ticks', 'render_shape', 'occludes')
function Assert-BlockSchema([string]$Id, $Def) {
    $bad = @($Def.Keys | Where-Object { $BlockKeys -notcontains $_ })
    if ($Def.Contains('properties')) { $bad += @($Def['properties'].Keys | Where-Object { $BlockPropertyKeys -notcontains $_ } | ForEach-Object { "properties.$_" }) }
    if ($bad.Count -gt 0) { throw "block $Id`: keys Horizon 7e4e421 refuses: $($bad -join ', ')" }
}

# Each block's shape table from its blockstate's variants: one box a state.
function Shapes([string]$Id) {
    $bs = Get-Content -Raw (Join-Path $art "blockstates\$Id.json") | ConvertFrom-Json -AsHashtable
    $names = @($bs.variants.Keys | Sort-Object)
    if ($names.Count -eq 1 -and $names[0] -eq '') {
        $model = $bs.variants[''].model -replace '^soulscraft:block/', ''
        return [ordered]@{ default = @(, (Model-Box $model)) }
    }
    $props = @(($names[0] -split ',') | ForEach-Object { ($_ -split '=')[0] } | Sort-Object)
    $values = [ordered]@{}
    foreach ($name in $names) {
        $v = $bs.variants[$name]
        $kv = @{}
        foreach ($p in $name -split ',') { $a = $p -split '='; $kv[$a[0]] = $a[1] }
        $key = ($props | ForEach-Object { $kv[$_] }) -join ','
        $model = $v.model -replace '^soulscraft:block/', ''
        $y = if ($v.Contains('y')) { [int]$v.y } else { 0 }
        $values[$key] = @(, (Turn-Box (Model-Box $model) $y))
    }
    return [ordered]@{ properties = $props; values = $values; default = @(, @(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)) }
}

# The blocks: one template with no states of its own (stone), the states the art's blockstates
# name (`part` of a tall piece, `on_slab`, a banner's `facing`, a brazier's `lit`), the outline of
# the models. The core glows (light 7). Breaking one segment of a tall piece breaks it all
# (soulscraft_altar); only the bottom drops the item (loot below).
$altarNames = [ordered]@{ altar_core = 'Ядро алтаря'; altar_pedestal = 'Подставка алтаря'; altar_slab = 'Плита алтаря'
    altar_socket = 'Гнездо модификатора'; altar_pillar = 'Колонна алтаря'; altar_brazier = 'Жаровня алтаря'; altar_obelisk = 'Обелиск алтаря' }
$altarEn = @{ altar_core = 'Altar Core'; altar_pedestal = 'Altar Pedestal'; altar_slab = 'Altar Slab'; altar_socket = 'Modifier Socket'
    altar_pillar = 'Altar Pillar'; altar_brazier = 'Altar Brazier'; altar_obelisk = 'Altar Obelisk' }
$blocks = [ordered]@{}
foreach ($id in $altarNames.Keys) { $blocks[$id] = $altarNames[$id] }
foreach ($m in $altar.modifiers) { $blocks[$m.banner] = $m.name }
$tall = @{ altar_pillar = @('bottom', 'top'); altar_obelisk = @('bottom', 'middle', 'top') }
foreach ($id in $blocks.Keys) {
    $banner = $id.StartsWith('banner_')
    $parts = if ($banner) { @('bottom', 'top') } elseif ($tall.Contains($id)) { $tall[$id] } else { $null }
    $states = [ordered]@{}; $default = [ordered]@{}
    if ($banner) { $states['facing'] = [ordered]@{ type = 'enum'; values = @('north', 'east', 'south', 'west') }; $default['facing'] = 'north' }
    if ($id -eq 'altar_brazier') { $states['lit'] = [ordered]@{ type = 'boolean' }; $default['lit'] = 'false' }
    if ($banner -or $parts -or $id -eq 'altar_brazier') { $states['on_slab'] = [ordered]@{ type = 'boolean' }; $default['on_slab'] = 'false' }
    if ($parts) { $states['part'] = [ordered]@{ type = 'enum'; values = $parts }; $default['part'] = 'bottom' }
    $props = [ordered]@{ hardness = $(if ($id -eq 'altar_core') { 3.5 } else { 2.0 }) }
    # Any tool or a hand gets the block back: stone's template wants a pickaxe for its drops, and
    # the break of `setblock ... destroy` (a tall piece's own removal) uses none.
    $props['requires_tool'] = $false
    if ($id -eq 'altar_core') { $props['light'] = 7 } else { $props['occludes'] = $false }
    if ($banner) { $props['sounds'] = 'minecraft:white_wool' }
    $def = [ordered]@{ template = 'minecraft:stone'; properties = $props; item = $true }
    if ($states.Count -gt 0) { $def['state_properties'] = $states; $def['default_state'] = $default }
    $def['shapes'] = Shapes $id
    Assert-BlockSchema $id $def
    Write-Text (Join-Path $Content "data\soulscraft\horizon\block\$id.json") (Json $def)
    # Loot: the block itself, from the bottom segment only.
    # Conditions name their kind under `type` (Horizon's loot reader, SRC/crates/horizon-items/src/items.rs:1829-1835;
    # vanilla's `condition` key there reads as no kind, which fails, and nothing drops).
    $cond = @([ordered]@{ type = 'minecraft:survives_explosion' })
    if ($parts) { $cond += [ordered]@{ type = 'minecraft:block_state_property'; block = "soulscraft:$id"; properties = [ordered]@{ part = 'bottom' } } }
    $loot = [ordered]@{ type = 'minecraft:block'; pools = @([ordered]@{ rolls = 1; entries = @([ordered]@{ type = 'minecraft:item'; name = "soulscraft:$id" }); conditions = $cond }) }
    Write-Text (Join-Path $Content "data\soulscraft\loot_table\blocks\$id.json") (Json $loot)
    Copy-File (Join-Path $art "blockstates\$id.json") (Join-Path $Content "assets\soulscraft\blockstates\$id.json")
    Copy-File (Join-Path $art "models\item\$id.json") (Join-Path $Content "assets\soulscraft\models\item\$id.json")
    Write-Text (Join-Path $Content "assets\soulscraft\items\$id.json") `
        (Json ([ordered]@{ model = [ordered]@{ type = 'minecraft:model'; model = "soulscraft:item/$id" } }))
    $lang["block.soulscraft.$id"] = $blocks[$id]
}
foreach ($f in Get-ChildItem (Join-Path $art 'models\block') -Filter *.json) {
    if ($f.Name -match '^(altar_|banner_)') { Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\models\block\$($f.Name)") }
}
foreach ($f in Get-ChildItem (Join-Path $art 'textures\block') -Filter *.png) {
    if ($f.Name -match '^(altar_|banner_)') { Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\block\$($f.Name)") }
}
foreach ($f in Get-ChildItem (Join-Path $art 'textures\item') -Filter *.png) {
    if ($f.Name -match '^(altar_|banner_|rotten_horn)') { Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\item\$($f.Name)") }
}
# Stone's template needs the right tool for drops: a pickaxe mines the altar.
Write-Text (Join-Path $Content 'data\minecraft\tags\block\mineable\pickaxe.json') `
    (Json ([ordered]@{ replace = $false; values = @($blocks.Keys | ForEach-Object { "soulscraft:$_" }) }))

# The summon item of the test event, Гнилой горн: soulscraft:rotten_horn (data/mobs.json
# events[0].item), drawn with the art's model of the same name. A plain item on the bone's template.
Write-Text (Join-Path $Content 'data\soulscraft\horizon\item\rotten_horn.json') `
    (Json ([ordered]@{ template = 'minecraft:bone'; max_stack_size = 16; rarity = 'uncommon' }))
Copy-File (Join-Path $art 'models\item\rotten_horn.json') (Join-Path $Content 'assets\soulscraft\models\item\rotten_horn.json')
Write-Text (Join-Path $Content 'assets\soulscraft\items\rotten_horn.json') `
    (Json ([ordered]@{ model = [ordered]@{ type = 'minecraft:model'; model = 'soulscraft:item/rotten_horn' } }))
$lang['item.soulscraft.rotten_horn'] = 'Гнилой горн'

# The survival slice (docs/design/10-survival.md): the soul shard, the hunt's currency, on the
# amethyst shard's template, and the summon horns of tiers 2-4 on the rotten horn's pattern.
# Drawn by assets/models/blocks/altar.py. id = template, stack, rarity, ru name, en name.
$survival = [ordered]@{
    soul_shard = @('minecraft:amethyst_shard', 64, 'common', 'Осколок души', 'Soul Shard')
    grave_horn = @('minecraft:bone', 16, 'uncommon', 'Горн мертвецов', 'Grave Horn')
    storm_horn = @('minecraft:bone', 16, 'rare', 'Горн бури', 'Storm Horn')
    abyss_horn = @('minecraft:bone', 16, 'epic', 'Горн бездны', 'Abyss Horn')
}
foreach ($si in $survival.Keys) {
    $d = $survival[$si]
    Write-Text (Join-Path $Content "data\soulscraft\horizon\item\$si.json") `
        (Json ([ordered]@{ template = $d[0]; max_stack_size = $d[1]; rarity = $d[2] }))
    Copy-File (Join-Path $art "models\item\$si.json") (Join-Path $Content "assets\soulscraft\models\item\$si.json")
    Copy-File (Join-Path $art "textures\item\$si.png") (Join-Path $Content "assets\soulscraft\textures\item\$si.png")
    Write-Text (Join-Path $Content "assets\soulscraft\items\$si.json") `
        (Json ([ordered]@{ model = [ordered]@{ type = 'minecraft:model'; model = "soulscraft:item/$si" } }))
    $lang["item.soulscraft.$si"] = $d[3]
}

# The tier rewards (docs/design/11-summons-rewards.md, «Предметы ступеней»): two weapons and a
# talisman, with 26.3 components over their template's. `attribute_modifiers` is a list, as in
# vanilla 26.3's items.json, and it replaces the template's list, so the weapons list their base
# damage and speed too (the player's own 1 damage and 4 attacks a second come on top). The
# weapons are repaired with soul shards. Sprites drawn by assets/models/blocks/rewards.py.
# id = template, rarity, ru name, en name, components.
function Modifier([string]$Type, [string]$Id, [double]$Amount, [string]$Slot) {
    [ordered]@{ type = $Type; id = $Id; amount = $Amount; operation = 'add_value'; slot = $Slot }
}
$byShard = [ordered]@{ items = 'soulscraft:soul_shard' }
$rewards = [ordered]@{
    revenant_blade = @('minecraft:iron_sword', 'uncommon', 'Клинок латника', "Revenant's Blade", [ordered]@{
            'minecraft:max_damage' = 1200
            'minecraft:attribute_modifiers' = @(
                (Modifier 'minecraft:attack_damage' 'minecraft:base_attack_damage' 6.0 'mainhand'),
                (Modifier 'minecraft:attack_speed' 'minecraft:base_attack_speed' -2.4 'mainhand'),
                (Modifier 'minecraft:armor' 'soulscraft:revenant_blade_armor' 2.0 'mainhand'))
            'minecraft:repairable' = $byShard })
    brute_maul = @('minecraft:iron_axe', 'rare', 'Молот громилы', "Brute's Maul", [ordered]@{
            'minecraft:max_damage' = 1800
            'minecraft:attribute_modifiers' = @(
                (Modifier 'minecraft:attack_damage' 'minecraft:base_attack_damage' 11.0 'mainhand'),
                (Modifier 'minecraft:attack_speed' 'minecraft:base_attack_speed' -3.2 'mainhand'),
                (Modifier 'minecraft:attack_knockback' 'soulscraft:brute_maul_knockback' 1.0 'mainhand'))
            'minecraft:repairable' = $byShard })
    abyss_heart = @('minecraft:heart_of_the_sea', 'epic', 'Сердце бездны', 'Abyss Heart', [ordered]@{
            'minecraft:attribute_modifiers' = @(
                (Modifier 'minecraft:max_health' 'soulscraft:abyss_heart_health' 4.0 'offhand')) })
}
function Item-Model([string]$Model) { [ordered]@{ type = 'minecraft:model'; model = "soulscraft:item/$Model" } }
# The brute's maul keeps its element model (assets/models/mobs/brute.py) where the brute holds it:
# the rig draws it with no holder. The player sees the sprite instead, in a slot, on the ground,
# in a frame and in the hand (the element maul is sized for the brute's fist).
$maulSprite = Item-Model 'brute_maul_sprite'
$itemModels = @{
    brute_maul = [ordered]@{ type = 'minecraft:select'; property = 'minecraft:display_context'
        cases = @([ordered]@{ when = @('gui', 'ground', 'fixed', 'on_shelf'); model = $maulSprite })
        fallback = [ordered]@{ type = 'minecraft:select'; property = 'minecraft:context_entity_type'
            cases = @([ordered]@{ when = 'minecraft:player'; model = $maulSprite })
            fallback = Item-Model 'brute_maul' } }
}
foreach ($ri in $rewards.Keys) {
    $d = $rewards[$ri]
    Write-Text (Join-Path $Content "data\soulscraft\horizon\item\$ri.json") `
        (Json ([ordered]@{ template = $d[0]; max_stack_size = 1; rarity = $d[1]; components = $d[4] }))
    $model = if ($itemModels.Contains($ri)) { $itemModels[$ri] } else { Item-Model $ri }
    Write-Text (Join-Path $Content "assets\soulscraft\items\$ri.json") (Json ([ordered]@{ model = $model }))
    $lang["item.soulscraft.$ri"] = $d[2]
}
foreach ($m in 'revenant_blade', 'brute_maul', 'brute_maul_sprite', 'abyss_heart') {
    Copy-File (Join-Path $art "models\item\$m.json") (Join-Path $Content "assets\soulscraft\models\item\$m.json")
    Copy-File (Join-Path $art "textures\item\$m.png") (Join-Path $Content "assets\soulscraft\textures\item\$m.png")
}

# The hunter's book (docs/design/08-guide-book.md, data/book.json `item`): a plain item on paper's
# template (a book's would be enchantable), drawn with the art's model (assets/models/blocks/altar.py), crafted cheaply from
# a book and rotten flesh. A right click opens its window (soulscraft_book).
$bookItem = (Get-Content -Raw -Encoding utf8 (Join-Path $Repo 'data\book.json') | ConvertFrom-Json).item -replace '^soulscraft:', ''
Write-Text (Join-Path $Content "data\soulscraft\horizon\item\$bookItem.json") `
    (Json ([ordered]@{ template = 'minecraft:paper'; max_stack_size = 1 }))
Copy-File (Join-Path $art "models\item\$bookItem.json") (Join-Path $Content "assets\soulscraft\models\item\$bookItem.json")
Copy-File (Join-Path $art "textures\item\$bookItem.png") (Join-Path $Content "assets\soulscraft\textures\item\$bookItem.png")
Write-Text (Join-Path $Content "assets\soulscraft\items\$bookItem.json") `
    (Json ([ordered]@{ model = [ordered]@{ type = 'minecraft:model'; model = "soulscraft:item/$bookItem" } }))
Write-Text (Join-Path $Content "data\soulscraft\recipe\$bookItem.json") `
    (Json ([ordered]@{ type = 'minecraft:crafting_shapeless'; category = 'misc'
        ingredients = @('minecraft:book', 'minecraft:rotten_flesh'); result = [ordered]@{ id = "soulscraft:$bookItem"; count = 1 } }))
$lang["item.soulscraft.$bookItem"] = 'Книга охотника'

# The recipes of the survival slice (docs/design/10-survival.md, «Рецепты»), vanilla 1.21.2+ JSON.
# Keys: G deepslate bricks, S soul shard, D diamond, Y gold ingot, and the rest named below.
function Write-Recipe([string]$Id, $Recipe) {
    Write-Text (Join-Path $Content "data\soulscraft\recipe\$Id.json") (Json $Recipe)
}
function Shaped([string]$Id, [string[]]$Pattern, $Key, [int]$Count = 1, [string]$Category = 'misc') {
    Write-Recipe $Id ([ordered]@{ type = 'minecraft:crafting_shaped'; category = $Category; pattern = $Pattern; key = $Key
            result = [ordered]@{ id = "soulscraft:$Id"; count = $Count } })
}
function Shapeless([string]$Id, [string[]]$Ingredients) {
    Write-Recipe $Id ([ordered]@{ type = 'minecraft:crafting_shapeless'; category = 'misc'; ingredients = $Ingredients
            result = [ordered]@{ id = "soulscraft:$Id"; count = 1 } })
}
$Bricks = 'minecraft:deepslate_bricks'; $Shard = 'soulscraft:soul_shard'
Shaped 'altar_slab' @('GG', 'GG') ([ordered]@{ G = $Bricks }) 4 'building'
Shaped 'altar_core' @('GSG', 'SDS', 'GSG') ([ordered]@{ G = $Bricks; S = $Shard; D = 'minecraft:diamond' }) 1 'building'
Shaped 'altar_pedestal' @(' S ', ' G ', 'GGG') ([ordered]@{ S = $Shard; G = $Bricks }) 1 'building'
Shaped 'altar_socket' @('GSG', 'GGG') ([ordered]@{ G = $Bricks; S = $Shard }) 2 'building'
Shaped 'altar_pillar' @('G', 'S', 'G') ([ordered]@{ G = $Bricks; S = $Shard }) 2 'building'
Shaped 'altar_brazier' @('SCS', 'GGG') ([ordered]@{ S = $Shard; C = 'minecraft:soul_campfire'; G = $Bricks }) 1 'building'
Shaped 'altar_obelisk' @(' O ', 'SAS', 'GGG') ([ordered]@{ O = 'minecraft:crying_obsidian'; S = $Shard; A = 'minecraft:amethyst_shard'; G = $Bricks }) 1 'building'
# A banner: a white banner, two shards and the emblem of its picture (10-survival.md, the emblem table).
$emblems = @{ weight = 'minecraft:anvil'; mercy = 'minecraft:golden_apple'; horde = 'minecraft:iron_sword'
    calm = 'minecraft:clock'; haste = 'minecraft:sugar'; rest = 'minecraft:campfire'; resilience = 'minecraft:shield'
    kin_undead = 'minecraft:skeleton_skull'; kin_beasts = 'minecraft:bone'; kin_slimes = 'minecraft:slime_ball'
    kin_flyers = 'minecraft:feather'; kin_water = 'minecraft:cod' }
foreach ($m in $altar.modifiers) {
    if (-not $emblems.Contains($m.id)) { throw "mod\sync.ps1: banner $($m.banner) has no emblem for its recipe" }
    Shapeless $m.banner @('minecraft:white_banner', $Shard, $Shard, $emblems[$m.id])
}
Shapeless 'rotten_horn' (@('minecraft:rotten_flesh') * 4 + @('minecraft:iron_ingot') * 4 + @('minecraft:bone'))
Shaped 'grave_horn' @('SYS', 'YDY', 'SYS') ([ordered]@{ S = $Shard; Y = 'minecraft:gold_ingot'; D = 'minecraft:diamond' })
# Any SoulsCraft head (the tag below, with the trophies); the event of tier 2 drops them.
Shaped 'storm_horn' @('RSR', 'DHD', 'RSR') ([ordered]@{ R = 'minecraft:blaze_rod'; S = $Shard; D = 'minecraft:diamond'; H = '#soulscraft:heads' })
Shaped 'abyss_horn' @('SNS', 'EBE', 'SSS') ([ordered]@{ S = $Shard; N = 'minecraft:netherite_scrap'; E = 'minecraft:ender_pearl'; B = 'soulscraft:brute_head' })

# The creative tab with the altar's blocks, banners, the horns, the shard, the tier rewards and the book.
$tabItems = @($blocks.Keys | ForEach-Object { "soulscraft:$_" }) + @('soulscraft:rotten_horn') +
    @('grave_horn', 'storm_horn', 'abyss_horn', 'soul_shard' | ForEach-Object { "soulscraft:$_" }) +
    @($rewards.Keys | ForEach-Object { "soulscraft:$_" }) + @("soulscraft:$bookItem")
Write-Text (Join-Path $Content 'data\soulscraft\horizon\creative_tab\altar.json') `
    (Json ([ordered]@{ title = 'itemGroup.soulscraft.altar'; icon = 'soulscraft:altar_core'; items = $tabItems }))
$lang['itemGroup.soulscraft.altar'] = 'SoulsCraft: алтарь'

$enNames = @{ revenant = 'Revenant'; stalker = 'Stalker'; marksman = 'Bone Marksman'; brute = 'Brute'
    shrike = 'Shrike'; lurker = 'Deep Lurker'; bog_slime = 'Bog Slime' }

# ---- Trophies (docs/design/09-trophies.md, iteration 3): heads, dioramas, armour pieces ----
# The heads and the dioramas are drawn by their block entities from assets/rigs/<mob>.trophy.json
# and the mob's rig (soulscraft_trophy_client); the diorama's plinth and scene are its block model.
$trophyTab = @()
$enTrophy = [ordered]@{}
$facing4 = [ordered]@{ type = 'enum'; values = @('north', 'east', 'south', 'west') }
foreach ($m in $mobs.mobs) {
    $id = $m.id
    $tf = Get-Content -Raw -Encoding utf8 (Join-Path $Repo "assets\rigs\$id.trophy.json") | ConvertFrom-Json
    Copy-File (Join-Path $Repo "assets\rigs\$id.trophy.json") (Join-Path $Content "assets\soulscraft\rigs\$id.trophy.json")
    # The server sends the diorama's particles from this copy, compiled in (soulscraft_rules::trophy::embedded_text).
    # The floor head: 16 rotations, a vanilla head's box; worn on the head (equippable, its item
    # model's `display.head`). Light passes it: the block entity draws the head in its cell.
    $rot = [ordered]@{ type = 'enum'; values = @(0..15 | ForEach-Object { "$_" }) }
    $head = [ordered]@{ template = 'minecraft:stone'
        properties = [ordered]@{ hardness = 1.0; requires_tool = $false; occludes = $false }
        item = $false
        state_properties = [ordered]@{ rotation = $rot }
        default_state = [ordered]@{ rotation = '0' }
        shapes = [ordered]@{ default = @(, @(0.25, 0.0, 0.25, 0.75, 0.5, 0.75)) } }
    Assert-BlockSchema "${id}_head_stand" $head
    Write-Text (Join-Path $Content "data\soulscraft\horizon\block\${id}_head_stand.json") (Json $head)
    # The wall head: vanilla's wall skull boxes (the back against the wall); its item is the head's.
    $wallBox = @{ north = @(0.25, 0.25, 0.5, 0.75, 0.75, 1.0); south = @(0.25, 0.25, 0.0, 0.75, 0.75, 0.5)
        east = @(0.0, 0.25, 0.25, 0.5, 0.75, 0.75); west = @(0.5, 0.25, 0.25, 1.0, 0.75, 0.75) }
    $wall = [ordered]@{ template = 'minecraft:stone'
        properties = [ordered]@{ hardness = 1.0; requires_tool = $false; occludes = $false }
        item = $false
        state_properties = [ordered]@{ facing = $facing4 }
        default_state = [ordered]@{ facing = 'north' }
        shapes = [ordered]@{ properties = @('facing'); values = [ordered]@{ north = @(, $wallBox.north); east = @(, $wallBox.east)
                south = @(, $wallBox.south); west = @(, $wallBox.west) }; default = @(, $wallBox.north) } }
    Assert-BlockSchema "${id}_head_mount" $wall
    Write-Text (Join-Path $Content "data\soulscraft\horizon\block\${id}_head_mount.json") (Json $wall)
    # The diorama: faces the player (state_for_placement), its own light, the outline of its model;
    # it collides only with the plinth (3 pixels), so its particles are not born inside a shape.
    $dio = [ordered]@{ template = 'minecraft:stone'
        properties = [ordered]@{ hardness = 1.5; requires_tool = $false; occludes = $false; light = [int]$tf.diorama.light; sounds = 'minecraft:oak_planks' }
        item = $true
        state_properties = [ordered]@{ facing = $facing4 }
        default_state = [ordered]@{ facing = 'north' }
        shapes = (Shapes "${id}_trophy")
        collision_shapes = [ordered]@{ default = @(, @(0.0, 0.0, 0.0, 1.0, 0.1875, 1.0)) } }
    Assert-BlockSchema "${id}_trophy" $dio
    Write-Text (Join-Path $Content "data\soulscraft\horizon\block\${id}_trophy.json") (Json $dio)
    # The art's blockstates and models keep the art's names (<mob>_head, <mob>_wall_head); the
    # blocks are <mob>_head_stand and <mob>_head_mount: Horizon makes any block named *_head a
    # vanilla skull block entity (soulscraft_rules::trophy::Kind).
    foreach ($pair in @(@("${id}_head", "${id}_head_stand"), @("${id}_wall_head", "${id}_head_mount"), @("${id}_trophy", "${id}_trophy"))) {
        $from = $pair[0]; $b = $pair[1]
        Copy-File (Join-Path $art "blockstates\$from.json") (Join-Path $Content "assets\soulscraft\blockstates\$b.json")
        Copy-File (Join-Path $art "models\block\$from.json") (Join-Path $Content "assets\soulscraft\models\block\$from.json")
        # Loot: either head form drops the head item, the diorama itself.
        $drop = if ($b -like '*_head_*') { "soulscraft:${id}_head" } else { "soulscraft:$b" }
        $loot = [ordered]@{ type = 'minecraft:block'; pools = @([ordered]@{ rolls = 1; entries = @([ordered]@{ type = 'minecraft:item'; name = $drop })
                    conditions = @([ordered]@{ type = 'minecraft:survives_explosion' }) }) }
        Write-Text (Join-Path $Content "data\soulscraft\loot_table\blocks\$b.json") (Json $loot)
    }
    # The head item: places the stand (`block`, turned by state_for_placement), worn on the
    # head (equippable, its model's display.head).
    Write-Text (Join-Path $Content "data\soulscraft\horizon\item\${id}_head.json") `
        (Json ([ordered]@{ template = 'minecraft:bone'; block = "soulscraft:${id}_head_stand"; rarity = 'uncommon'
            components = [ordered]@{ 'minecraft:equippable' = [ordered]@{ slot = 'head' } } }))
    foreach ($b in "${id}_head", "${id}_trophy") {
        Copy-File (Join-Path $art "models\item\$b.json") (Join-Path $Content "assets\soulscraft\models\item\$b.json")
        Write-Text (Join-Path $Content "assets\soulscraft\items\$b.json") `
            (Json ([ordered]@{ model = [ordered]@{ type = 'minecraft:model'; model = "soulscraft:item/$b" } }))
        $trophyTab += "soulscraft:$b"
    }
    Copy-File (Join-Path $art "textures\item\${id}_head.png") (Join-Path $Content "assets\soulscraft\textures\item\${id}_head.png")
    $lang["item.soulscraft.${id}_head"] = "Голова: $($m.name)"
    $lang["block.soulscraft.${id}_head_stand"] = "Голова: $($m.name)"
    $lang["block.soulscraft.${id}_head_mount"] = "Голова: $($m.name)"
    $lang["block.soulscraft.${id}_trophy"] = "Диорама: $($m.name)"
    $enTrophy["item.soulscraft.${id}_head"] = "$($enNames[$id]) Head"
    $enTrophy["block.soulscraft.${id}_head_stand"] = "$($enNames[$id]) Head"
    $enTrophy["block.soulscraft.${id}_head_mount"] = "$($enNames[$id]) Head"
    $enTrophy["block.soulscraft.${id}_trophy"] = "$($enNames[$id]) Diorama"
}
foreach ($f in Get-ChildItem (Join-Path $art 'textures\block') -Filter *.png) {
    if ($f.Name -match '^(dio_|trophy_)') { Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\block\$($f.Name)") }
}
# The diorama's own particle types (API 103): assets/<ns>/particles/<type>.json and their sprites.
foreach ($f in Get-ChildItem (Join-Path $art 'particles') -Filter *.json) {
    Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\particles\$($f.Name)")
}
foreach ($f in Get-ChildItem (Join-Path $art 'textures\particle') -Filter *.png) {
    Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\particle\$($f.Name)")
}
# The armour pieces (the bestiary's loot, data/mobs.json): decorative, no armour points. Helmets and
# hoods are worn on the head and drawn from their item model's `display.head`; the chestplate is
# a vanilla-shaped chestplate with its own texture (equipment soulscraft:rusty, humanoid layer).
$armour = [ordered]@{}
foreach ($m in $mobs.mobs) { foreach ($l in @($m.loot)) { if ($l) { $armour[($l.item -replace '^soulscraft:', '')] = $l.name } } }
$enArmour = @{ rusty_helmet = 'Rusty Helmet'; rusty_chestplate = 'Rusty Chestplate'; stalker_hood = "Stalker's Hood"
    marksman_hood = "Marksman's Torn Hood"; brute_helmet = "Brute's Horned Helmet" }
foreach ($a in $armour.Keys) {
    $equip = if ($a -like '*chestplate') { [ordered]@{ slot = 'chest'; asset_id = 'soulscraft:rusty' } } else { [ordered]@{ slot = 'head' } }
    Write-Text (Join-Path $Content "data\soulscraft\horizon\item\$a.json") `
        (Json ([ordered]@{ template = 'minecraft:bone'; max_stack_size = 1; rarity = 'uncommon'; components = [ordered]@{ 'minecraft:equippable' = $equip } }))
    Copy-File (Join-Path $art "models\item\$a.json") (Join-Path $Content "assets\soulscraft\models\item\$a.json")
    Copy-File (Join-Path $art "textures\item\$a.png") (Join-Path $Content "assets\soulscraft\textures\item\$a.png")
    Write-Text (Join-Path $Content "assets\soulscraft\items\$a.json") `
        (Json ([ordered]@{ model = [ordered]@{ type = 'minecraft:model'; model = "soulscraft:item/$a" } }))
    $lang["item.soulscraft.$a"] = $armour[$a]
    $enTrophy["item.soulscraft.$a"] = $enArmour[$a]
    $trophyTab += "soulscraft:$a"
}
foreach ($f in Get-ChildItem (Join-Path $art 'equipment') -Filter *.json) {
    Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\equipment\$($f.Name)")
}
foreach ($f in Get-ChildItem (Join-Path $art 'textures\entity\equipment\humanoid') -Filter *.png) {
    Copy-File $f.FullName (Join-Path $Content "assets\soulscraft\textures\entity\equipment\humanoid\$($f.Name)")
}
Write-Text (Join-Path $Content 'data\soulscraft\horizon\creative_tab\trophies.json') `
    (Json ([ordered]@{ title = 'itemGroup.soulscraft.trophies'; icon = 'soulscraft:revenant_head'; items = $trophyTab }))
$lang['itemGroup.soulscraft.trophies'] = 'SoulsCraft: трофеи'
$enTrophy['itemGroup.soulscraft.trophies'] = 'SoulsCraft: Trophies'
# Every head item, for the storm horn's recipe (10-survival.md, «Рецепты»).
Write-Text (Join-Path $Content 'data\soulscraft\tags\item\heads.json') `
    (Json ([ordered]@{ replace = $false; values = @($mobs.mobs | ForEach-Object { "soulscraft:$($_.id)_head" }) }))

Write-Text (Join-Path $Content 'assets\soulscraft\lang\ru_ru.json') (Json $lang)
$en = [ordered]@{}
foreach ($m in $mobs.mobs) { $en["entity.soulscraft.$($m.id)"] = $enNames[$m.id] }
$en['item.soulscraft.rusty_sword'] = 'Rusty Sword'
$en['item.soulscraft.stalker_sickle'] = "Stalker's Sickle"
$en['effect.soulscraft.bleeding'] = 'Bleeding'
foreach ($id in $blocks.Keys) {
    $en["block.soulscraft.$id"] = if ($altarEn.Contains($id)) { $altarEn[$id] } else {
        'Banner of ' + (Get-Culture).TextInfo.ToTitleCase(($id -replace '^banner_', '' -replace '_', ' ')) }
}
$en['item.soulscraft.rotten_horn'] = 'Rotten Horn'
foreach ($si in $survival.Keys) { $en["item.soulscraft.$si"] = $survival[$si][4] }
foreach ($ri in $rewards.Keys) { $en["item.soulscraft.$ri"] = $rewards[$ri][3] }
$en["item.soulscraft.$bookItem"] = "Hunter's Book"
$en['itemGroup.soulscraft.altar'] = 'SoulsCraft: Altar'
foreach ($k in $enTrophy.Keys) { $en[$k] = $enTrophy[$k] }
Write-Text (Join-Path $Content 'assets\soulscraft\lang\en_us.json') (Json $en)

if ($script:Changed.Count -eq 0) {
    Write-Host 'Copies match their sources.'
} elseif ($Check) {
    $script:Changed | ForEach-Object { Write-Host "  out of date: $_" }
    throw "Run .\mod\sync.ps1: $($script:Changed.Count) copies differ from their sources"
} else {
    $script:Changed | ForEach-Object { Write-Host "  wrote $_" }
}
