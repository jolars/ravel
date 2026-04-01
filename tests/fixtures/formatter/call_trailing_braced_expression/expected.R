with(data, {
  col
})

with(data, {
  col
})

with(data, {
  col
})

with(
  data,
  # A comment
  {
    col
  }
)

with(
  data, # Prevents flattening
  {
    col
  }
)

with(data, expr = {
  col
})

with(data, foo = "bar", {
  col
})

# Not trailing, stays expanded
with(
  data,
  {
    col
  },
  foo = "bar"
)

# Breaks and fully expands due to line length
with(
  my_long_list_my_long_list_my_long_list_my_long_list_long_long_long_long_long_list,
  {
    col
  }
)

# Collapses with empty braces
with(data, {})

with(data, {
  # dangling
})

# Collapses with empty braces
fn({})

fn({
  # dangling
})

fn({
  1 + 1
})

fn(a = {
  1 + 1
})

# The first argument here breaks, causing everything to fully expand
fn(
  {
    1 + 1
  },
  {
    1 + 1
  }
)

# Hole prevents `{` from looking like the last expression, so everything expands
fn(
  x,
  {
    1 + 1
  },
)
