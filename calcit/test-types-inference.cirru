
{} (:package |test-types-inference)
  :configs $ {} (:init-fn |test-types-inference.main/main!) (:reload-fn |test-types-inference.main/reload!)
  :files $ {}
    |test-types-inference.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println "|Testing type inference..."
              test-list-inference
              test-optional-inference
              test-fn-inference
              test-map-inference
              test-set-inference
              test-ref-inference
              test-record-inference
              test-type-ref-combos

        |test-list-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-list-inference ()
              let
                  nested $ [] ([] 1 2) ([] 3)
                assert-type nested (:: :list (:: :list :number))
                &inspect-type nested
                let
                    inner (&list:nth nested 0)
                  &inspect-type inner
                  let
                      val (&list:nth inner 0)
                    &inspect-type val

        |test-optional-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-optional-inference ()
              let
                  opt 10
                assert-type opt (:: :optional :number)
                &inspect-type opt
              let
                  opt-nil nil
                assert-type opt-nil (:: :optional :string)
                &inspect-type opt-nil

        |test-fn-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-fn-inference ()
              let
                  f (fn (x) (+ x 1))
                hint-fn f (:: :fn (:number) :number)
                &inspect-type f

        |test-map-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map-inference ()
              let
                  m ({})
                assert-type m (:: :map :string :number)
                let
                    m2 (&map:assoc m |b 2)
                    m3 (&map:dissoc m2 |a)
                    m4 (&map:get m2 |b)
                  &inspect-type m2
                  &inspect-type m3
                  &inspect-type m4

        |test-set-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-set-inference ()
              let
                  s (#{})
                assert-type s (:: :set :number)
                let
                    xs (&set:to-list s)
                  &inspect-type s
                  &inspect-type xs

        |test-ref-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-ref-inference ()
              let
                  r (atom 1)
                assert-type r (:: :ref :number)
                let
                    x (&atom:deref r)
                  &inspect-type r
                  &inspect-type x

        |Person $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Person (:name :string) (:age :number) (:address Address)

        |Address $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Address (:city :string)

        |Status $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Status
              :ok :number
              :err :string

        |Job $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct Job (:title :string) (:status Status)

        |PersonWrap $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum PersonWrap
              :person Person
              :none

        |Outcome $ %{} :CodeEntry (:doc |)
          :code $ quote
            defenum Outcome
              :status Status
              :none

        |test-record-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-record-inference ()
              let
                  addr $ %{} (new-record :Address :city) (:city |sh)
                  p $ %{} (new-record :Person :name :age :address) (:name |n) (:age 20) (:address addr)
                assert-type p Person
                &inspect-type p

        |test-type-ref-combos $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-type-ref-combos ()
              let
                  addr $ %{} (new-record :Address :city) (:city |sh)
                  person $ %{} (new-record :Person :name :age :address) (:name |n) (:age 20) (:address addr)
                  job $ %{} (new-record :Job :title :status) (:title |dev) (:status (%:: Status :ok 1))
                assert-type person Person
                assert-type job Job
                &inspect-type person
                &inspect-type job
                let
                    wrapped $ %:: PersonWrap :person person
                    outcome $ %:: Outcome :status (%:: Status :ok 2)
                  assert-type wrapped PersonWrap
                  assert-type outcome Outcome
                  &inspect-type wrapped
                  &inspect-type outcome

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-types-inference.main)
