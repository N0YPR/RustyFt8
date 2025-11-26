program test_sync8
  integer, parameter :: NPTS=180000
  real :: dd(NPTS)
  real :: candidate(3,600)
  real :: sbase(2048)
  integer :: ncand
  integer :: nfa, nfb
  real :: syncmin
  integer :: nfqso, maxcand
  integer :: i

  ! Read WAV file and populate dd array
  call read_wav_file('tests/test_data/210703_133430.wav', dd, NPTS)

  ! Set parameters matching RustyFt8 test
  nfa = 200       ! freq_min
  nfb = 3500      ! freq_max
  syncmin = 0.3   ! sync_min
  nfqso = 1500    ! QSO frequency (not used for finding all candidates)
  maxcand = 200   ! max_candidates

  ! Call sync8 (WSJT-X coarse sync)
  call sync8(dd, NPTS, nfa, nfb, syncmin, nfqso, maxcand, candidate, ncand, sbase)

  ! Print results
  print *, 'Found ', ncand, ' candidates'
  print *, ''
  print *, 'Freq(Hz)  Time(s)  Sync'
  print *, '================================'
  do i = 1, ncand
    print '(F8.1, F8.3, F8.3)', candidate(1,i), candidate(2,i), candidate(3,i)
  enddo

end program test_sync8

subroutine read_wav_file(filename, data, npts)
  character*(*) :: filename
  integer :: npts
  real :: data(npts)
  integer*2 :: iwave(npts)
  integer :: i

  ! Open and read WAV file (simple 16-bit PCM reader)
  open(10, file=filename, status='old', access='stream', form='unformatted')

  ! Skip 44-byte WAV header
  read(10, pos=45) iwave

  ! Convert to real
  do i = 1, npts
    data(i) = real(iwave(i))
  enddo

  close(10)
end subroutine read_wav_file
