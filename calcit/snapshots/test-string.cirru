
{} (:package |test-string)
  :configs $ {} (:init-fn |test-string.main/main!) (:reload-fn |test-string.main/reload!)
  :files $ {}
    |test-string.main $ {}
      :ns $ quote
        ns test-string.main $ :require
          [] util.core :refer $ [] inside-eval:
      :defs $ {}

        |test-str $ quote
          defn test-str ()
            assert= (&str-concat |a |b) |ab
            assert= (&str-concat 1 2) |12
            assert= (str |a |b |c) |abc
            assert= (str 1 2 3) |123
            assert= (type-of (&str 1)) :string
            assert=
              replace "|this is a" |is |IS
              , "|thIS IS a"
            assert=
              split "|a,b,c" "|,"
              [] |a |b |c
            assert=
              split-lines "|a\nb\nc"
              [] |a |b |c
            assert=
              split |a中b文c |
              [] |a |中 |b |文 |c
            assert= 4
              count |good
            assert= |56789 $ substr |0123456789 5
            assert= |567 $ substr |0123456789 5 8
            assert= | $ substr |0123456789 10
            assert= | $ substr |0123456789 9 1
            assert= -1 $ compare-string |a |b
            assert= 1 $ compare-string |b |a
            assert= 0 $ compare-string |a |a

            assert-detect identity $ < |a |b
            assert-detect identity $ < |aa |ab
            assert-detect not $ > |aa |ab

        |test-includes $ quote
          fn ()
            log-title "|Testing includes"

            assert= true $ includes? |abc |abc
            assert= false $ includes? |abd |abc

            assert= 3 $ str-find |0123456 |3
            assert= 3 $ str-find |0123456 |34
            assert= 0 $ str-find |0123456 |01
            assert= 4 $ str-find |0123456 |456
            assert= -1 $ str-find |0123456 |98

            assert= true $ starts-with? |01234 |0
            assert= true $ starts-with? |01234 |01
            assert= false $ starts-with? |01234 |12

            assert= true $ ends-with? |01234 |34
            assert= true $ ends-with? |01234 |4
            assert= false $ ends-with? |01234 |23

            assert= |abc $ strip-prefix |ababc |ab
            assert= |0abc $ strip-prefix |0abc |ab

            assert= |aba $ strip-suffix |ababc |bc
            assert= |abc0 $ strip-suffix |abc0 |bc

        |test-parse $ quote
          fn ()
            assert= 0 $ parse-float |0

        |test-trim $ quote
          fn ()
            assert= | $ trim "|    "
            assert= |1 $ trim "|  1  "
            assert= |1 $ trim "|\n1\n"

            assert= | $ trim "|______" |_
            assert= |1 $ trim "|__1__" |_

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |test-format $ quote
          fn ()
            log-title "|Testing format"

            assert= |1.2346 $ format-number 1.23456789 4
            assert= |1.235 $ format-number 1.23456789 3
            assert= |1.23 $ format-number 1.23456789 2
            assert= |1.2 $ format-number 1.23456789 1

            inside-eval:

              ; TODO not stable
              ; assert= "|({} (:c ([] 3)) (:a 1) (:b |2) (:d ({} (([] 1 2) 3))))"
                pr-str $ {}
                  :a 1
                  :b |2
                  :c $ [] 3
                  :d $ {}
                    ([] 1 2) 3

            let
                Person $ new-record 'Person :name :age
                edn-demo "|%{} Person (age 23) (name |Chen)"
              assert=
                pr-str $ %{} Person (:name |Chen) (:age 23)
                , "|(%{} Person (age 23) (name |Chen))"
              assert= edn-demo
                trim $ write-cirru-edn $ %{} Person (:name |Chen) (:age 23)

              assert=
                parse-cirru-edn edn-demo
                %{} Person (:name |Chen) (:age 23)

              assert= 'a
                parse-cirru-edn "|do 'a"
              assert= "|[] 'a"
                trim $ write-cirru-edn $ [] 'a

              assert= "|do nil"
                trim $ write-cirru-edn nil

              assert= "|do 's"
                trim $ write-cirru-edn 's

              assert= (escape "|\n") "|\"\\n\""
              assert= (escape "|\t") "|\"\\t\""
              assert= (escape "|a") "|\"a\""

        |test-char $ quote
          fn ()
            log-title "|Test char"

            assert= 97 $ get-char-code |a
            assert= 27721 $ get-char-code |汉

            assert= |a $ nth |abc 0
            assert= |b $ nth |abc 1
            assert= |a $ first |abc
            assert= |c $ last |abc
            assert= nil $ first |
            assert= nil $ last |

        |test-re $ quote
          fn ()
            log-title "|Test regular expression"

            assert= true $ re-matches |2 |\d
            assert= true $ re-matches |23 |\d+
            assert= false $ re-matches |a |\d

            assert= 1 $ re-find-index |a1 |\d
            assert= -1 $ re-find-index |aa |\d

            assert= ([] |1 |2 |3) $ re-find-all |123 |\d
            assert= ([] |123) $ re-find-all |123 |\d+
            assert= ([] |1 |2 |3) $ re-find-all |1a2a3 |\d+
            assert= ([] |1 |2 |34) $ re-find-all |1a2a34 |\d+

        |test-whitespace $ quote
          fn ()
            log-title "|Test blank?"

            assert-detect identity $ blank? |
            assert-detect identity $ blank? "\""
            assert-detect identity $ blank? "| "
            assert-detect identity $ blank? "|  "
            assert-detect identity $ blank? "|\n"
            assert-detect identity $ blank? "|\n "
            assert-detect not $ blank? |1
            assert-detect not $ blank? "| 1"
            assert-detect not $ blank? "|1 "

        |test-lisp-style $ quote
          fn ()
            log-title "|Test lisp style"

            assert=
              format-to-lisp $ quote (defn f1 (x) (+ x y))
              , "|(defn f1 (x) (+ x y))"

            assert=
              format-to-lisp $ quote $ nil? nil
              , "|(nil? nil)"

        |test-methods $ quote
          defn test-methods ()
            log-title "|Testing methods"

            assert= 3 (.count |abc)

        |main! $ quote
          defn main! ()
            log-title "|Testing str"
            test-str

            test-includes

            log-title "|Testing parse"
            test-parse

            log-title "|Testing trim"
            test-trim

            test-format

            test-char

            test-re

            test-whitespace

            test-lisp-style

            test-methods

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
