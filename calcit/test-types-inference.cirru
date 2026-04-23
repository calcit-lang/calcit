
{} (:about "|Machine-generated snapshot. AI AGENTS: never edit this file directly — changes will be overwritten on recompile. Inspect via `cr query`; modify via `cr edit` / `cr tree`. MANDATORY first step: run `cr docs agents --full`.") (:package |test-types-inference)
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
            defn main! () (println "|Testing type inference...") (test-list-inference) (test-optional-inference) (test-fn-inference) (test-map-inference) (test-set-inference) (test-ref-inference) (test-record-inference) (test-type-ref-combos) (test-generics-identity)
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
                &inspect-type inner
                let
                    val $ &list:nth inner 0
                  &inspect-type val
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
                &inspect-type m2
                &inspect-type m3
                &inspect-type m4
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
          :examples $ []
        |test-record-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-record-inference () $ let
                addr $ %{} Address (:city |sh)
                p $ %{} Person (:name |n) (:age 20) (:address addr)
              assert-type p $ :: Person
              &inspect-type p
          :examples $ []
        |test-ref-inference $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-ref-inference () $ let
                r $ atom 1
              assert-type r $ :: :ref :number
              let
                  x $ &atom:deref r
                &inspect-type r
                &inspect-type x
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
