# Scope generated lowering checks to one receiver block

Review found that the JavaScript regression expressions could start matching at one generated receiver temporary and satisfy their primitive checks from a later lowering block. The String `last` assertion was in fact using a module without a String-last fixture, so its previous success demonstrated the false-positive risk.

The check now extracts each generated receiver IIFE and requires all expected primitives to occur within the same block. List and String `last` use their respective real fixture modules. A new `dynamic-last-compat` definition with an explicit `Dynamic -> Option<Dynamic>` contract provides the opposite assertion: generated JavaScript must retain `invoke_method("last", value)` and must not invent a typed receiver temporary.
