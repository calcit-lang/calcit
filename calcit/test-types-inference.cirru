
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-types-inference)
  :configs $ {} (:init-fn |test-types-inference.main/main!) (:reload-fn |test-types-inference.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-types-inference.main $ %{} :FileEntry
      :defs $ {}
        |Address $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct Address $ :city :string
          :examples $ []
        |Job $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct Job (:title :string) (:status Status)
          :examples $ []
        |Outcome $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum Outcome (:status Status) (:none)
          :examples $ []
        |Person $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct Person (:name :string) (:age :number)
              :address $ :: Address
          :examples $ []
        |PersonWrap $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum PersonWrap
              :person $ :: Person
              :none
          :examples $ []
        |Status $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defenum Status (:ok :number) (:err :string)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () (println "|Testing type inference...") (test-list-inference) (test-optional-inference) (test-count-inference) (test-fn-inference) (test-map-inference) (test-set-inference) (test-ref-inference) (test-record-inference) (test-type-ref-combos) (test-generics-identity)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |test-fn-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-fn-inference () $ let
                f $ fn (x) (+ x 1)
              hint-fn f $ {}
                :args $ [] (:: 'x :number)
                :return :number
              &inspect-type f
          :examples $ []
        |test-generics-identity $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-generics-identity () $ let
                n $ identity 42
                s $ identity |hello
              assert-type n :number
              assert-type s :string
              &inspect-type n
              &inspect-type s
          :examples $ []
        |test-list-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-list-inference () $ let
                nested $ [] ([] 1 2) ([] 3)
              assert-type nested $ :: :list (:: :list :number)
              &inspect-type nested
              let
                  inner $ &list:nth nested 0
                assert-type inner $ :: :list :number
                &inspect-type inner
              let
                  val $ &list:nth ([] 1 2 3) 0
                assert-type val :number
                &inspect-type val
              let
                  xs $ [] 1 2 3
                  rest-xs $ rest xs
                assert-type rest-xs $ :: :optional (:: :list :number)
                &inspect-type rest-xs
          :examples $ []
        |test-map-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-map-inference () $ let
                m $ {}
              assert-type m $ :: :map :string :number
              let
                  m2 $ &map:assoc m |b 2
                  m3 $ &map:dissoc m2 |a
                  m4 $ &map:get m2 |b
                  m5 $ merge m2 $ {} (:c 3)
                &inspect-type m2
                &inspect-type m3
                &inspect-type m4
                assert-type m5 $ :: :map :string :number
                &inspect-type m5
          :examples $ []
        |test-optional-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-optional-inference ()
              let
                  opt 10
                assert-type opt $ :: :optional :number
                &inspect-type opt
              let
                  opt-nil nil
                assert-type opt-nil $ :: :optional :string
                &inspect-type opt-nil
              let
                  xs $ [] 1 2
                  empty-xs $ empty xs
                assert-type empty-xs $ :: :optional (:: :list :number)
                &inspect-type empty-xs
              let
                  empty-nil $ empty nil
                assert-type empty-nil $ :: :optional :string
                &inspect-type empty-nil
              let
                  rest-nil $ rest nil
                assert-type rest-nil $ :: :optional :string
                &inspect-type rest-nil
              let
                  tail $ rest |abc
                assert-type tail $ :: :optional :string
                &inspect-type tail
              let
                  first-char $ &str:first |abc
                  missing-first $ &str:first |
                assert-type first-char $ :: :optional :string
                assert-type missing-first $ :: :optional :string
                &inspect-type first-char
                &inspect-type missing-first
              let
                  list-first $ first ([] 1 2 3)
                  string-first $ first |abc
                  empty-list-first $ first ([])
                  empty-str-first $ first |
                  nil-first $ first nil
                  tuple-first $ first (:: :a 1 2)
                assert-type list-first $ :: :optional :dynamic
                assert-type string-first $ :: :optional :dynamic
                assert-type empty-list-first $ :: :optional :dynamic
                assert-type empty-str-first $ :: :optional :dynamic
                assert-type nil-first $ :: :optional :dynamic
                assert-type tuple-first $ :: :optional :dynamic
                &inspect-type list-first
                &inspect-type string-first
                &inspect-type empty-list-first
                &inspect-type empty-str-first
                &inspect-type nil-first
                &inspect-type tuple-first
              let
                  list-last $ last ([] 1 2 3)
                  empty-list-last $ last ([])
                  nil-last $ last nil
                assert-type list-last $ :: :optional :dynamic
                assert-type empty-list-last $ :: :optional :dynamic
                assert-type nil-last $ :: :optional :dynamic
                &inspect-type list-last
                &inspect-type empty-list-last
                &inspect-type nil-last
              let
                  nth-char $ &str:nth |abc 1
                  missing-char $ &str:nth |abc 9
                assert-type nth-char $ :: :optional :string
                assert-type missing-char $ :: :optional :string
                &inspect-type nth-char
                &inspect-type missing-char
              let
                  hit-index $ &str:find-index |abc |b
                  miss-index $ &str:find-index |abc |z
                assert-type hit-index $ :: :optional :number
                assert-type miss-index $ :: :optional :number
                &inspect-type hit-index
                &inspect-type miss-index
              let
                  parsed-ok $ parse-float |1.5
                  parsed-bad $ parse-float |oops
                assert= 1.5 parsed-ok
                assert= nil parsed-bad
                assert-type parsed-ok $ :: :optional :number
                assert-type parsed-bad $ :: :optional :number
                &inspect-type parsed-ok
                &inspect-type parsed-bad
              let
                  list-hit $ get ([] 1 2 3) 1
                  list-miss $ get ([] 1 2 3) 9
                  string-hit $ get |abc 1
                  map-hit $ get ({} (:a 1)) :a
                  map-miss $ get ({} (:a 1)) :b
                  nil-hit $ get nil :a
                assert-type list-hit $ :: :optional :number
                assert-type list-miss $ :: :optional :number
                assert-type string-hit $ :: :optional :string
                assert-type map-hit $ :: :optional :number
                assert-type map-miss $ :: :optional :number
                assert-type nil-hit $ :: :optional :dynamic
                &inspect-type list-hit
                &inspect-type list-miss
                &inspect-type string-hit
                &inspect-type map-hit
                &inspect-type map-miss
                &inspect-type nil-hit
              let
                  nested-hit $ get-in ([] ({} (:a 1))) ([] 0 :a)
                  nested-miss $ get-in ({} (:p ({} (:name |n)))) ([] :p :age)
                  nil-nested $ get-in nil ([] :a)
                assert-type nested-hit $ :: :optional :number
                assert-type nested-miss $ :: :optional :dynamic
                assert-type nil-nested $ :: :optional :dynamic
                &inspect-type nested-hit
                &inspect-type nested-miss
                &inspect-type nil-nested
          :examples $ []
        |test-record-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-record-inference () $ let
                addr $ %{} Address (:city |sh)
                p $ %{} Person (:name |n) (:age 20) (:address addr)
              assert-type p $ :: Person
              &inspect-type p
              let
                  name-v $ &record:get p :name
                assert-type name-v $ :: :optional :dynamic
                &inspect-type name-v
              let
                  top-name-v $ get p :name
                  top-miss-v $ get p :email
                  city-v $ get-in p ([] :address :city)
                  city-miss-v $ get-in p ([] :address :zip)
                assert-type top-name-v $ :: :optional :string
                assert-type top-miss-v $ :: :optional :dynamic
                assert-type city-v $ :: :optional :string
                assert-type city-miss-v $ :: :optional :dynamic
                &inspect-type top-name-v
                &inspect-type top-miss-v
                &inspect-type city-v
                &inspect-type city-miss-v
          :examples $ []
        |test-ref-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-ref-inference () $ let
                r $ atom 1
              assert-type r $ :: :ref :number
              let
                  x $ &atom:deref r
                assert-type x :number
                &inspect-type r
                &inspect-type x
          :examples $ []
        |test-count-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-count-inference ()
              assert-type (count nil) :number
              assert-type (count ([] 1 2 3)) :number
              assert-type (count |abc) :number
          :examples $ []
        |test-set-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-set-inference () $ let
                s $ #{}
              assert-type s $ :: :set :number
              let
                  xs $ &set:to-list s
                &inspect-type s
                &inspect-type xs
          :examples $ []
        |test-type-ref-combos $ %{} :CodeEntry (:doc |) (:schema nil)
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
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns test-types-inference.main)
