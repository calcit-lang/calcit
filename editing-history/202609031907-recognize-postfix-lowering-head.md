# Recognize postfix method heads in lowering evidence

Review found that `query type-at` treated the first item of every list as its callable head. In postfix source syntax such as `receiver .method`, however, the second item is the method head. That mismatch could label the receiver as the source/lowered callable and misclassify remaining dynamic dispatch.

The lowering classifier now selects a Method in either of the two legal head positions, preferring the ordinary leading position when present. A regression test covers unchanged postfix dynamic dispatch and verifies that both reported heads remain `.show`.
