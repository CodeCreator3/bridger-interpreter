## Concept questions
A few sentences each, in your own words. These are about the code you wrote. Point at your own implementation where a question asks you to, rather than repeating the textbook's answers.

```
$ cat deps.brg
relation needs : (String, String);
relation uses : (String, String);

rule uses(a, b) :- needs(a, b);
rule uses(a, c) :- needs(a, b) and uses(b, c);

fn main() {
    let edges = [("app", "ui"), ("ui", "gfx"), ("app", "net"), ("net", "io")];
    for e in edges { add needs(e.0, e.1); }
    println(solutions uses("app", ?m));
    println(uses("io", "app"));
    for uses(?m, "io") { println(m); }
}

$ cargo run -q --features m6 -- deps.brg
["gfx", "io", "net", "ui"]
false
app
net
```

**solutions uses("app", ?m) in deps.brg (from above) is ["gfx", "io", "net", "ui"]. Using your engine, explain how uses("app", "io") gets derived: which round of your fixpoint first produces it, which rule fires, and what substitution your unify builds for each generator. How does your loop decide that it has reached the fixpoint? Why does the same loop terminate on a graph with a cycle, such as an extra fact needs("io", "app")?**
uses("app", "io") is derived with this rule `rule uses(a, c) :- needs(a, b) and uses(b, c);` because a can be bound to app, b can be bound to net, and c can be bound to io. This builds off of needs having the relations (app, net) and (net, io) and uses having the other rule `rule uses(a, b) :- needs(a, b);`. Its the second round because uses(net, io) has to be bound first. For `rule uses(a, c) :- needs(a, b) and uses(b, c);` there are two generators: needs(a,b) and uses(b,c) the generated values are what I already mentioned. My loop decides it has reached the fixpoint like this: it checks all the rules, for each rule it adds facts to the database, if the facts were already present for every rule, then we're done. On a graph with a circle, we cycle once, have all the facts, and exit.

**solutions needs(?a, ?) is ["app", "net", "ui"], three values from four facts. Point at where your engine cuts each match down to the named holes, removes duplicates, and sorts. Why must the anonymous hole be cut away before duplicates are removed? Why does each ? need a fresh variable name, and what would solutions needs(?, ?) return if your engine gave every anonymous hole the same name?**
That happens around lines 198 of engine.rs. If the binding was labeled as an unnamed hole, its removed (not added to `named`). The subs are then sorted with the value compare fn. The list has dedup called which removes duplicates. The anonymous holes must be cut away before duplicates are removed because anonymous holes won't be part of the solution. Each ? needs a fresh variable name to distinguish them from each other. needs(?, ?) returns whether there is a needs relation where both sides are the same (in that scenario).

**rule leaf(a) :- size(a, n) and m > cap; is Unbound (m), but n > cap is accepted, and so is { let m = n * 2; m > cap }. Walk through your check_range_restriction on all three: which names free_vars gives for each filter, and which test lets each name through or rejects it. Why does the language insist that every head variable be bound by a generator? What would a rule like rule uses(a, c) :- needs(a, b); have to mean if it were allowed?**
for the first one, this code checks for unbound vars:

```
for term in rule.head.terms.clone() {
        match term {
            Term::Var(name) => {
                if !names.contains(&name) {
                    return Err(RuleError::Unbound {
                        var: name,
                        span: rule.head.span,
                    });
                }
            }
            _ => (),
        }
    }
```

for the second one, n is bound because it came from a generator.
for the third one, m is bound by the let.

free_vars:
1: `rule leaf(a) :- size(a, n) and m > cap;` - for `m > cap` gives `m` and `cap`
2: `rule leaf(a) :- size(a, n) and n > cap;` - for `n > cap` gives `n` and `cap`
3: `rule leaf(a) :- size(a, n) and { let m = n * 2; m > cap };` - for `{ let m = n * 2; m > cap }` gives `n` and `cap`

My testing code looks like this:
```
if names.contains(&var) {
    continue;
}
if is_native(&var) {
    continue;
}
match program.decls.get(&var) {
    Some(_) => continue,
    None => {
        return Err(RuleError::Unbound {
            var,
            span: term.span(),
        })
    }
}
```
it allows free vars that are given by generators with `names.contains(&var)`, natives with `is_native(&var)`, and declarations with `match program.decls.get(&var)`

If we allowed `rule uses(a, c) :- needs(a, b);`, then I'm not sure where we would get a values for b. Filters should reduce the number of substitutions that work, not add new substitutions with new variables.




```
$ cat hops.brg
relation link : (String, String);
relation near : (String, String);

fn hops(n: Int) -> Bool = if n <= 0 { true } else { hops(n - 1) };

rule near(a, b) :- link(a, b) and hops(2);

fn main() {
    println("start");
    add link("a", "b");
    println(solutions near(?a, ?b));
}

$ cargo run -q --features m6 -- hops.brg
  × `hops` is recursive, so a filter calling it might not terminate
   ╭─[hops.brg:6:35]
 5 │
 6 │ rule near(a, b) :- link(a, b) and hops(2);
   ·                                   ───┬───
   ·                                      ╰── here
 7 │
   ╰────
```
**hops.brg (above) is rejected as Recursive, although hops(2) always returns. Show where your check_pure finds the cycle and why it reports Recursive rather than Impure. Then, for the filter note(a), where note calls println, explain how your code builds the witness [note, println], and how it computes the impure set I as a least fixpoint over the program's functions.**
Around line 430 of `relations\check.rs` we iterate over the functions in the filter and call `calls_itself` on that fn. That fn essentially finds all subexpressions of that fn and checks if there are any calls to the fn itself.
It reports recursive rather than impure because technically recursion isn't impure by itself; however, its still not allowed in a filter, so it needs to be rejected.
For the witness of note(a), my code calls witness_in which is given the fn to check for purity and a list of all impure fns. It looks at all sub expressions of the fn and for variables, it checks if they are in the list of impure fns. If so, it gets the witness from the impure fn list element that was found.
To build the impure set, I check all fns in the program directly for impurities by calling witness_in (starting with an empty impure set), which finds all the fns that are directly doing impure things like calling print. Then it keeps looping with the updated impure set calling witness_in which adds fns that call impure fns. This keeps going until the impure set is no longer being updated.

**The purity check already rejects every filter that queries, yet your engine also keeps a materializing flag. Point at where you set and clear it in solutions, and explain why it must be cleared before an error from the rounds is returned. Using the textbook's p/q/r program from the chapter's design problems, describe what your engine would do, run without the checks, if the flag were not there.**
in `engine.rs` in the solutions fn, we set `materializing` to true before calling `run_rounds` which finds all the solutions, then we set `materializing` to false
It must be cleared before an error is returned, because if an error is returned during rule evaluation, it will be return out of the function skipping the setting of `materializing` to false, which will cause every later query to be stuck.
For p/q/r without the checks and without the materializing flag, the engine would start deriving all facts to find `solutions p(?x)` and in the process need to derive all facts to compute the query `not r(x)` which would require computing `p(x)` cycling in a loop causing stack overflow.

## How you implemented it
**Walk through your solutions from start to finish: how you turn the query's arguments into terms, how you compute the fixpoint (the starting facts, one round, and firing one rule through its generators and filters), and how you build the answer. Say how your Subst::unify handles a variable already bound, and how it leaves the original substitution unchanged on failure. Naming the provided tools you used (RelationDb, Subst, compare, type_of, env.root(), Control) is encouraged.**
Solutions procedure:
- if materializing, raise QueryInFilter
- determine the relations name by looking it up
- for each argument of the query, create a term
    - if it is an expression, evaluate that and make it a Term::Val
    - if it is a named hole, make it a Term::Var
    - if it is an unnamed hole, make it a Term::Var with the name "anonX" (each one gets a unique X)
- check the arity of these terms against the relations columns
- add starting facts
    - if the facts have already been memoized into self.db.derived, just use those
    - otherwise compute the base facts from relations and rules
- compute the rest of the facts and apply filters until no new facts are added 
    - I set materializing to true before starting this and false after
    - I made sure to store the error and report after materializing is reset
    - for each iteration, we iterate over all rules, and for each rule we run through all the conjucts in the body
        - if the conjuct is a generator, we create substitutions that match that relation
        - if its a filter, we remove subs that don't pass the filter
- once we get an answer from this, we get just named holes, sort the subs, and remove duplicates

In unify if a var is already bound, we check if they are unequal, if so we return None (the positions disagreed), if they are equal, we continue. It clones self, so its not actually changing itself its returning a new sub. 

I used the relational database, I used Subst, I used compare, I used type_of, and I used Control.

**Describe how your check_pure decides what a call reaches: how you tell a local from a top-level function or a native, how you treat map, filter, and fold callbacks, and which of the provided helpers (subexprs, free_vars, bound_names) you used and where.**
Determining what a call reaches:
- how you tell a local from a top-level function or a native:
    - to determine if its a local, we check against bound names
    - to determine if its a top-level function, we compare it to the set of top-level functions provided by program
    - to determine if its a native, we compare it to the set of natives
- how you treat map, filter, and fold callbacks
    - my witness_in fn (which checks the purity of functions), checks for map filter and fold, assigning the number 1 for map and filter and 2 for fold (to get the index of where the inner var is), then we look at that var, if its an impure fn, a relation, or a global, then we report impurity
- I did use subexprs, but realized that it only gets surface level subexprs; it's not a recursive search. So I made all_subexprs which gets every single expression I used it when checking for impure exprs like deref
- I used free_vars to make sure filters were only reading from generators or vars they created
- I used bound_names to check if a callee was a local in a term (making it not a relation)

**Point out anything that was tricky, a bug you fixed, or a design choice you made. If you used an assistant, say what you had it do and how you checked its work.**
- Something tricky was getting the correct witness values particularly with let bindings that bound impure fns
- After writing the first pass of the whole m6 program by myself, debugging the rust errors myself, then working through 46 of the 64 tests with an assistant as my TA, the deadline was approaching. I decided to have the assistant work through the remaining 18 tests itself and give me a summary of what it changed. Unfortunately, after getting the program to pass 16 tests, the model started getting confused and wouldn't give me a coherant response, so I made my program pass the remaining 2 tests myself. Because of these setbacks, my submission ended up being a day late (overall m6 probably took around 8-10 hours to complete).