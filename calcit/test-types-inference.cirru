
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
            defrecord Person :name :age

        |test-record-inference $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-record-inference ()
              let
                  p $ %{} Person (:name |n) (:age 20)
                assert-type p (:: :typeref :test-types-inference.main :Person)
                &inspect-type p

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns test-types-inference.main)
