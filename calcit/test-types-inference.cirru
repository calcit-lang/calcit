
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-types-inference) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-types-inference.main/main!) (:mode :native) (:reload-fn 'test-types-inference.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-types-inference.main $ %{} 'FileEntry
      :defs $ {}
        |Address $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Address $ :city 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |Job $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Job (:title 'String) (:status Status)
          :examples $ []
          :schema $ :: 'Dynamic
        |Outcome $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defenum Outcome (:status Status) (:none)
          :examples $ []
          :schema $ :: 'Dynamic
        |Person $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defstruct Person (:name 'String) (:age 'Number)
              :address $ :: Address
          :examples $ []
          :schema $ :: 'Dynamic
        |PersonWrap $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defenum PersonWrap
              :person $ :: Person
              :none
          :examples $ []
          :schema $ :: 'Dynamic
        |Status $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defenum Status (:ok 'Number) (:err 'String)
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (println "|Testing type inference...") (test-list-inference) (test-optional-inference) (test-count-inference) (test-fn-inference) (test-map-inference) (test-set-inference) (test-ref-inference) (test-struct-inference) (test-type-ref-combos) (test-generics-identity)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |test-count-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-count-inference ()
              assert-type
                count $ [] 1 2 3
                , 'Number
              assert-type (count |abc) 'Number
          :examples $ []
          :schema $ :: 'Dynamic
        |test-fn-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-fn-inference () $ let
                f $ fn (x) (+ x 1)
              hint-fn f $ {}
                :args $ [] 'Number
                :return 'Number
              assert-type (f 1) 'Number
              &inspect-type f
          :examples $ []
          :schema $ :: 'Dynamic
        |test-generics-identity $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-generics-identity () $ let
                n $ identity 42
                s $ identity |hello
              assert-type n 'Number
              assert-type s 'String
              &inspect-type n
              &inspect-type s
          :examples $ []
          :schema $ :: 'Dynamic
        |test-list-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-list-inference () $ let
                nested $ [] ([] 1 2) ([] 3)
              assert-type nested $ :: 'List (:: 'List 'Number)
              &inspect-type nested
              let
                  inner $ &list:nth nested 0
                assert-type inner $ :: 'List 'Number
                &inspect-type inner
              let
                  val $ &list:nth ([] 1 2 3) 0
                assert-type val 'Number
                &inspect-type val
              let
                  xs $ [] 1 2 3
                  rest-xs $ rest xs
                assert-type rest-xs $ :: 'List 'Number
                &inspect-type rest-xs
          :examples $ []
          :schema $ :: 'Dynamic
        |test-map-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-map-inference () $ let
                m $ {}
              assert-type m $ :: 'Map 'String 'Number
              let
                  m2 $ &map:assoc m |b 2
                  m3 $ &map:dissoc m2 |a
                  m4 $ &map:get m2 |b
                  m5 $ merge m2
                    {} $ :c 3
                &inspect-type m2
                &inspect-type m3
                &inspect-type m4
                assert-type m5 $ :: 'Map 'String 'Number
                &inspect-type m5
          :examples $ []
          :schema $ :: 'Dynamic
        |test-optional-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-optional-inference ()
              let
                  opt 10
                assert-type opt $ :: 'Optional 'Number
                &inspect-type opt
              let
                  opt-nil nil
                assert-type opt-nil $ :: 'Optional 'String
                &inspect-type opt-nil
              let
                  xs $ [] 1 2
                  empty-xs $ empty xs
                assert-type empty-xs $ :: 'List 'Number
                &inspect-type empty-xs
              let
                  tail $ rest |abc
                assert-type tail 'String
                &inspect-type tail
              let
                  first-char $ &str:first |abc
                  missing-first $ &str:first |
                assert-type first-char $ :: 'Optional 'String
                assert-type missing-first $ :: 'Optional 'String
                &inspect-type first-char
                &inspect-type missing-first
              let
                  list-first $ first ([] 1 2 3)
                  string-first $ first |abc
                  empty-list-first $ first ([])
                  empty-str-first $ first |
                assert-type list-first $ :: 'Option 'Number
                assert-type string-first $ :: 'Option 'String
                assert-type empty-list-first $ :: 'Option 'Dynamic
                assert-type empty-str-first $ :: 'Option 'String
                &inspect-type list-first
                &inspect-type string-first
                &inspect-type empty-list-first
                &inspect-type empty-str-first
              let
                  list-last $ last ([] 1 2 3)
                  empty-list-last $ last ([])
                assert-type list-last $ :: 'Option 'Number
                assert-type empty-list-last $ :: 'Option 'Dynamic
                &inspect-type list-last
                &inspect-type empty-list-last
              let
                  nth-char $ &str:nth |abc 1
                  missing-char $ &str:nth |abc 9
                assert-type nth-char $ :: 'Optional 'String
                assert-type missing-char $ :: 'Optional 'String
                &inspect-type nth-char
                &inspect-type missing-char
              let
                  raw-hit-index $ &str:find-index |abc |b
                  raw-miss-index $ &str:find-index |abc |z
                  hit-index $ str-find-index |abc |b
                  miss-index $ str-find-index |abc |z
                assert= 1 raw-hit-index
                assert= -1 raw-miss-index
                assert= (%some 1) hit-index
                assert= (%none) miss-index
                assert-type raw-hit-index 'Number
                assert-type raw-miss-index 'Number
                assert-type hit-index $ :: 'Option 'Number
                assert-type miss-index $ :: 'Option 'Number
                &inspect-type raw-hit-index
                &inspect-type raw-miss-index
                &inspect-type hit-index
                &inspect-type miss-index
              let
                  parsed-ok $ parse-float |1.5
                  parsed-bad $ parse-float |oops
                assert= (%ok 1.5) parsed-ok
                assert= (%err |oops) parsed-bad
                assert-type parsed-ok $ :: 'Result 'Number 'String
                assert-type parsed-bad $ :: 'Result 'Number 'String
                &inspect-type parsed-ok
                &inspect-type parsed-bad
              let
                  found $ find ([] 1 2 3)
                    fn (x) (> x 1)
                  missing $ find ([] 1 2 3)
                    fn (x) (> x 9)
                  found-index $ find-index ([] 1 2 3)
                    fn (x) (> x 1)
                  missing-index $ index-of ([] 1 2 3) 9
                assert-type found $ :: 'Option 'Number
                assert-type missing $ :: 'Option 'Number
                assert-type found-index $ :: 'Option 'Number
                assert-type missing-index $ :: 'Option 'Number
                &inspect-type found
                &inspect-type missing
                &inspect-type found-index
                &inspect-type missing-index
              let
                  last-hit $ .find-last ([] 1 2 3)
                    fn (x) (> x 1)
                  last-index $ .find-last-index ([] 1 2 3)
                    fn (x) (> x 1)
                  last-position $ .last-index-of ([] 1 2 1) 1
                  list-max $ .max ([] 1 2 3)
                  set-min $ .min (#{} 1 2 3)
                  string-index $ .find-index |abc |b
                assert-type last-hit $ :: 'Option 'Number
                assert-type last-index $ :: 'Option 'Number
                assert-type last-position $ :: 'Option 'Number
                assert-type list-max $ :: 'Option 'Number
                assert-type set-min $ :: 'Option 'Number
                assert-type string-index $ :: 'Option 'Number
                &inspect-type last-hit
                &inspect-type last-index
                &inspect-type last-position
                &inspect-type list-max
                &inspect-type set-min
                &inspect-type string-index
              let
                  list-hit $ get ([] 1 2 3) 1
                  list-miss $ get ([] 1 2 3) 9
                  string-hit $ get |abc 1
                  map-hit $ get
                    {} $ :a 1
                    , :a
                  map-miss $ get
                    {} $ :a 1
                    , :b
                assert-type list-hit $ :: 'Option 'Number
                assert-type list-miss $ :: 'Option 'Number
                assert-type string-hit $ :: 'Option 'String
                assert-type map-hit $ :: 'Option 'Number
                assert-type map-miss $ :: 'Option 'Number
                &inspect-type list-hit
                &inspect-type list-miss
                &inspect-type string-hit
                &inspect-type map-hit
                &inspect-type map-miss
              let
                  nested-hit $ get-in
                    [] $ {} (:a 1)
                    [] 0 :a
                  nested-miss $ get-in
                    {} $ :p
                      {} $ :name |n
                    [] :p :age
                  nil-nested $ get-in nil ([] :a)
                assert= (%some 1) nested-hit
                assert= (%none) nested-miss
                assert= (%none) nil-nested
                assert-type nested-hit $ :: 'Option 'Number
                assert-type nested-miss $ :: 'Option 'Dynamic
                assert-type nil-nested $ :: 'Option 'Dynamic
                &inspect-type nested-hit
                &inspect-type nested-miss
                &inspect-type nil-nested
          :examples $ []
          :schema $ :: 'Dynamic
        |test-ref-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-ref-inference () $ let
                r $ atom 1
              assert-type r $ :: 'Ref 'Number
              let
                  x $ &atom:deref r
                assert-type x 'Number
                &inspect-type r
                &inspect-type x
          :examples $ []
          :schema $ :: 'Dynamic
        |test-set-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-set-inference () $ let
                s $ #{}
              assert-type s $ :: 'Set 'Number
              let
                  xs $ &set:to-list s
                &inspect-type s
                &inspect-type xs
          :examples $ []
          :schema $ :: 'Dynamic
        |test-struct-inference $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-struct-inference () $ let
                addr $ %{} Address (:city |sh)
                p $ %{} Person (:name |n) (:age 20) (:address addr)
              assert-type p $ :: Person
              &inspect-type p
              let
                  name-v $ &struct:get p :name
                assert-type name-v String
                &inspect-type name-v
              let
                  top-name-v $ :name p
                  city-v $ :city (:address p)
                assert-type top-name-v String
                assert-type city-v String
                &inspect-type top-name-v
                &inspect-type city-v
          :examples $ []
          :schema $ :: 'Dynamic
        |test-type-ref-combos $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-type-ref-combos () $ let
                addr $ %{} Address (:city |sh)
                person $ %{} Person (:name |n) (:age 20) (:address addr)
                job $ %{} Job (:title |dev)
                  :status $ %:: Status :ok 1
              assert-type person $ :: Person
              assert-type job $ :: Job
              &inspect-type person
              &inspect-type job
              let
                  wrapped $ %:: PersonWrap :person person
                  outcome $ %:: Outcome :status (%:: Status :ok 2)
                assert-type wrapped PersonWrap
                assert-type outcome Outcome
                &inspect-type wrapped
                &inspect-type outcome
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote (ns test-types-inference.main)
