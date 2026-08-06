
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-edn) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-edn.main/main!) (:mode :native) (:reload-fn 'test-edn.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-edn.main $ %{} :FileEntry
      :defs $ {}
        |A $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct A $ :a 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |A-typed-person $ %{} :CodeEntry (:doc |)
          :code $ quote
            def A-typed-person $ parse-cirru-edn-as "|%{} :Person (:age 23) (:name |Top)" Person
          :examples $ []
          :schema $ :: 'Dynamic
        |Box $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Box ([] 'T) (:value 'T)
          :examples $ []
          :schema $ :: 'Dynamic
        |DemoEnum $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum DemoEnum (:ok) (:err 'String)
          :examples $ []
          :schema $ :: 'Dynamic
        |Person $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Person (:name 'String) (:age 'Number)
          :examples $ []
          :schema $ :: 'Dynamic
        |Team $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Team $ :members (:: 'List Person)
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing edn") (test-edn) (test-edn-comment)
              inside-eval: $ test-symbol
              test-atom
              test-typed-edn
              test-imported-typed-edn
              test-top-level-typed-edn
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! $
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        |test-atom $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-atom () (log-title "|Testing atom to edn")
              let
                  a $ parse-cirru-edn "|atom 1"
                println "|Check a" a
                assert= true $ ref? a
                assert= 1 $ deref a
                assert= "|atom 1" $ trim (format-cirru-edn a)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-edn () $ let
                edn-demo "|%{} :Person (:age 23) (:name |Chen)"
              ; "no stable order"
              assert=
                count $ to-lispy-string
                  %{} Person (:name |Chen) (:age 23)
                count "|(%{} :Person (:name |Chen) (:age 23))"
              ; "no stable order"
              assert= (count edn-demo)
                count $ trim
                  format-cirru-edn $ %{} Person (:name |Chen) (:age 23)
              assert= (parse-cirru-edn edn-demo)
                %{} Person (:name |Chen) (:age 23)
              assert= 'a $ parse-cirru-edn "|do 'a"
              assert=
                {} $ :code
                  cirru-quote $ + 1 2 3
                parse-cirru-edn "|{} $ :code $ quote $ + 1 2 3"
              assert= (:: :a 1) (parse-cirru-edn "|:: :a 1")
              assert= :cirru-quote $ type-of (parse-cirru "|a b")
              let
                  tree $ parse-cirru "|a b"
                  t0 $ &cirru-nth tree 0
                  t00 $ &cirru-nth t0 0
                assert= :list $ &cirru-type t0
                assert= :leaf $ &cirru-type t00
              println $ parse-cirru "|a b"
              println $ &cirru-nth (parse-cirru "|a b") 0
              assert= "|{} $ :code\n  quote $ + 1 2 3" $ trim
                format-cirru-edn $ {}
                  :code $ :: 'quote ([] |+ |1 |2 |3)
              assert= "|{} $ :code\n  quote $ + 1 2 3" $ trim
                format-cirru-edn $ {}
                  :code $ cirru-quote (+ 1 2 3)
              assert= "|[] 'a" $ trim
                format-cirru-edn $ [] 'a
              assert= "|do nil" $ trim (format-cirru-edn nil)
              assert= "|do 's" $ trim (format-cirru-edn 's)
              assert=
                trim $ format-cirru-edn
                  {} (:a 1) (:b 2) (:Z 3) (:D 4)
                , "|{} (:D 4) (:Z 3) (:a 1) (:b 2)"
              assert=
                trim $ format-cirru-edn
                  {} (|a 1) (|b 2) (|Z 3) (|D 4)
                , "|{} (|D 4) (|Z 3) (|a 1) (|b 2)"
              assert=
                trim $ format-cirru-edn
                  {} (:c 2) (:a1 0)
                    :b $ [] 1 2
                    :a 1
                , "|{} (:a 1) (:a1 0) (:c 2)\n  :b $ [] 1 2"
              assert= "|:: :&core-list-methods $ [] 1 2 3" $ trim
                format-cirru-edn $ :: &core-list-methods ([] 1 2 3)
              assert= "|:: :test" $ trim
                format-cirru-edn $ :: :test
              assert= "|:: :test :a :b" $ trim
                format-cirru-edn $ :: :test :a :b
              let
                  enum-ok $ parse-cirru-edn "|%:: :DemoEnum :ok"
                    {} $ :DemoEnum DemoEnum
                assert= :ok $ &enum:nth enum-ok 0
                assert= "|%:: :DemoEnum :ok" $ trim (format-cirru-edn enum-ok)
              let
                  enum-err $ parse-cirru-edn "|%:: :DemoEnum :err |oops"
                    {} $ :DemoEnum DemoEnum
                assert= :err $ &enum:nth enum-err 0
                assert= "|%:: :DemoEnum :err |oops" $ trim (format-cirru-edn enum-err)
              assert= "|do \"|a b\"" $ trim (format-cirru-edn "|a b")
              assert= "|do |hello" $ trim (format-cirru-edn |hello)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-edn-comment $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-edn-comment () (log-title "|Testing edn comment")
              assert=
                [] 1 2 (; comment) 3
                parse-cirru-edn "|[] 1 2 (; comment) 3"
              assert=
                {} (:a 1) (:b 2) (; comment)
                parse-cirru-edn "|{} (:a 1) (:b 2)"
              assert= (:: :a 1) (parse-cirru-edn "|:: :a (; comment) 1")
          :examples $ []
          :schema $ :: 'Dynamic
        |test-imported-typed-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-imported-typed-edn () $ assert=
              %{} External $ :label |linked
              parse-cirru-edn-as "|%{} :External (:label |linked)" External
          :examples $ []
          :schema $ :: 'Dynamic
        |test-symbol $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-symbol () (log-title "|Testing symbol to edn")
              assert= (&extract-code-into-edn 'aa)
                {} (:ns |test-edn.main) (:kind :symbol) (:val |aa) (:at-def |test-symbol)
                  :location $ [] 4 1 1
              let
                  data $ [] 1 2
                    &{} :a 1 :b $ :: :t 3 |a true
                assert= data $ eval (&data-to-code data)
              let
                  d $ %{}? A
                assert= true $ struct? d
              let
                  data $ #{} 1 2 3
                assert= data $ eval (&data-to-code data)
              let
                  d $ [] :t 's
                assert= d $ eval (&data-to-code d)
              let
                  code $ quote (+ 1 2)
                assert= code $ eval (&data-to-code code)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-top-level-typed-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-top-level-typed-edn () $ assert=
              %{} Person (:name |Top) (:age 23)
              , A-typed-person
          :examples $ []
          :schema $ :: 'Dynamic
        |test-typed-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-typed-edn () $ let
                person $ parse-cirru-edn-as "|%{} :Person (:age 23) (:name |Ada)" Person
                people $ parse-cirru-edn-as "|[] $ %{} :Person (:age 23) (:name |Ada)" (:: 'List Person)
                team $ parse-cirru-edn-as "|%{} :Team (:members ([] (%{} :Person (:age 23) (:name |Ada))))" Team
                boxed $ parse-cirru-edn-as "|%{} :Box (:value |hello)" (:: Box 'String)
                enum-value $ parse-cirru-edn-as "|%:: :DemoEnum :err |oops" DemoEnum
              assert=
                %{} Person (:name |Ada) (:age 23)
                , person
              assert= ([] person) people
              assert=
                %{} Team $ :members ([] person)
                , team
              assert=
                %{} Box $ :value |hello
                , boxed
              assert= :err $ &enum:nth enum-value 0
              assert= true $ try
                do
                  parse-cirru-edn-as "|[] $ %{} :Person (:age |old) (:name |Ada)" $ :: 'List Person
                  , false
                fn (error) true
              assert= true $ try
                do (parse-cirru-edn-as "|%{} :Person (:name |Ada)" Person) false
                fn (error) true
              assert= true $ try
                do
                  parse-cirru-edn-as "|#{} (%{} :Person (:age 23) (:name |Ada)) (%{} :Person (:name |Ada) (:age 23))" $ :: 'Set Person
                  , false
                fn (error) true
              assert= true $ try
                do
                  parse-cirru-edn-as "|{} ((%{} :Person (:age 23) (:name |Ada)) |first) ((%{} :Person (:name |Ada) (:age 23)) |second)" $ :: 'Map Person 'String
                  , false
                fn (error) true
              assert= (#{} 1)
                parse-cirru-edn-as "|#{} 1 1" $ :: 'Set 'Number
              assert=
                {} $ :a |second
                parse-cirru-edn-as "|{} (:a |first) (:a |second)" $ :: 'Map 'Tag 'String
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-edn.main $ :require
            util.core :refer $ inside-eval: log-title
            test-edn.schema :refer $ External
    |test-edn.schema $ %{} :FileEntry
      :defs $ {}
        |External $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct External $ :label 'String
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-edn.schema)
